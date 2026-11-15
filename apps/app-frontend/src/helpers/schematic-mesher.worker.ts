/// <reference lib="webworker" />
/**
 * Builds the preview geometry off the main thread: the schematic is put into
 * one grid, and only faces that can be seen become triangles. The result is
 * three big vertex/index buffers (opaque, cutout, translucent) instead of one
 * object per block.
 */
import { blockIndexAt, isAirLike, type SchematicRegion } from './litematic'
import type { BlockRender, MeshLayerData, MeshResult } from './schematic-types'

interface MeshRequest {
	regions: SchematicRegion[]
	blocks: Record<string, BlockRender>
	maxCells: number
	maxFaces: number
}

class LayerBuilder {
	positions = new Float32Array(3 * 4096)
	uvs = new Float32Array(2 * 4096)
	colors = new Uint8Array(4 * 4096)
	indices = new Uint32Array(6 * 1024)
	vertices = 0
	indexCount = 0

	private grow<T extends Float32Array | Uint8Array | Uint32Array>(array: T, needed: number): T {
		if (needed <= array.length) return array
		let size = array.length
		while (size < needed) size *= 2
		const bigger = new (array.constructor as new (length: number) => T)(size)
		bigger.set(array)
		return bigger
	}

	quad(p: number[], uv: number[], r: number, g: number, b: number) {
		this.positions = this.grow(this.positions, (this.vertices + 4) * 3)
		this.uvs = this.grow(this.uvs, (this.vertices + 4) * 2)
		this.colors = this.grow(this.colors, (this.vertices + 4) * 4)
		this.indices = this.grow(this.indices, this.indexCount + 6)

		this.positions.set(p, this.vertices * 3)
		this.uvs.set(uv, this.vertices * 2)
		for (let i = 0; i < 4; i++) {
			const o = (this.vertices + i) * 4
			this.colors[o] = r
			this.colors[o + 1] = g
			this.colors[o + 2] = b
			this.colors[o + 3] = 255
		}
		const v = this.vertices
		this.indices.set([v, v + 1, v + 2, v, v + 2, v + 3], this.indexCount)
		this.vertices += 4
		this.indexCount += 6
	}

	finish(): MeshLayerData {
		return {
			positions: this.positions.slice(0, this.vertices * 3),
			uvs: this.uvs.slice(0, this.vertices * 2),
			colors: this.colors.slice(0, this.vertices * 4),
			indices: this.indices.slice(0, this.indexCount),
		}
	}
}

// The six directions: neighbor offset, shading, and the quad corners
// (x, y, z with y: 0 = bottom of the block, 1 = top), counter-clockwise from outside.
const FACES = [
	{ dx: 1, dy: 0, dz: 0, shade: 0.8, kind: 'side', corners: [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]] },
	{ dx: -1, dy: 0, dz: 0, shade: 0.8, kind: 'side', corners: [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]] },
	{ dx: 0, dy: 1, dz: 0, shade: 1, kind: 'top', corners: [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]] },
	{ dx: 0, dy: -1, dz: 0, shade: 0.5, kind: 'bottom', corners: [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]] },
	{ dx: 0, dy: 0, dz: 1, shade: 0.65, kind: 'side', corners: [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]] },
	{ dx: 0, dy: 0, dz: -1, shade: 0.65, kind: 'side', corners: [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]] },
] as const
// u/v choice for each of the four corners
const UV_PATTERN = [[0, 0], [1, 0], [1, 1], [0, 1]] as const

const MAX_STEP = 64

function build(request: MeshRequest): { type: 'done'; result: MeshResult } | { type: 'empty' | 'too-large' } {
	const { regions, blocks, maxCells, maxFaces } = request

	// Bounds around all regions; negative sizes extend in the negative direction.
	const boxes = regions.map((region) => ({
		region,
		sx: Math.abs(region.size.x),
		sy: Math.abs(region.size.y),
		sz: Math.abs(region.size.z),
		minX: region.position.x + (region.size.x < 0 ? region.size.x + 1 : 0),
		minY: region.position.y + (region.size.y < 0 ? region.size.y + 1 : 0),
		minZ: region.position.z + (region.size.z < 0 ? region.size.z + 1 : 0),
	}))
	if (boxes.length === 0) return { type: 'empty' }
	const minX = Math.min(...boxes.map((b) => b.minX))
	const minY = Math.min(...boxes.map((b) => b.minY))
	const minZ = Math.min(...boxes.map((b) => b.minZ))
	const dx = Math.max(...boxes.map((b) => b.minX + b.sx)) - minX
	const dy = Math.max(...boxes.map((b) => b.minY + b.sy)) - minY
	const dz = Math.max(...boxes.map((b) => b.minZ + b.sz)) - minZ
	// Block "states" by shape: a slab, a carpet and a full block of the same
	// material are different states. State 0 means empty.
	const stateRender: (BlockRender | null)[] = [null]
	const stateY0: number[] = [0]
	const stateY1: number[] = [0]
	const stateFull: boolean[] = [false]
	const stateName: string[] = ['']
	const stateIds = new Map<string, number>()

	const stateFor = (name: string, properties: Record<string, string>): number => {
		if (isAirLike(name)) return 0
		const render = blocks[name]
		if (!render) return 0
		const id = name.replace(/^minecraft:/, '')
		let y0 = 0
		let y1 = 1
		if (id.endsWith('_slab')) {
			if (properties.type === 'bottom') y1 = 0.5
			else if (properties.type === 'top') y0 = 0.5
		} else if (id.endsWith('_carpet') || id.endsWith('_pressure_plate')) {
			y1 = 1 / 16
		} else if (id === 'snow') {
			y1 = Math.min(1, (parseInt(properties.layers ?? '1', 10) || 1) / 8)
		}
		const key = `${name}|${y0}|${y1}`
		let state = stateIds.get(key)
		if (state === undefined) {
			state = stateRender.length
			stateIds.set(key, state)
			stateRender.push(render)
			stateY0.push(y0)
			stateY1.push(y1)
			stateFull.push(y0 === 0 && y1 === 1)
			stateName.push(name)
		}
		return state
	}

	// Huge schematics (hundreds of millions of cells) are never put into one
	// full-size grid: it is built coarser right away, merging 2x2x2, 4x4x4, ...
	// cells while the file is read.
	let baseStep = 1
	while (
		Math.ceil(dx / baseStep) * Math.ceil(dy / baseStep) * Math.ceil(dz / baseStep) > maxCells &&
		baseStep < MAX_STEP
	) {
		baseStep *= 2
	}
	const shift = Math.log2(baseStep)
	const bx = Math.ceil(dx / baseStep)
	const by = Math.ceil(dy / baseStep)
	const bz = Math.ceil(dz / baseStep)
	if (bx * by * bz > maxCells) return { type: 'too-large' }

	const grid = new Uint16Array(bx * by * bz)
	for (const box of boxes) {
		const paletteStates = box.region.palette.map((entry) => stateFor(entry.name, entry.properties))
		for (let y = 0; y < box.sy; y++) {
			const cy = (box.minY + y - minY) >> shift
			for (let z = 0; z < box.sz; z++) {
				const cz = (box.minZ + z - minZ) >> shift
				const row = cz * bx + cy * bx * bz
				for (let x = 0; x < box.sx; x++) {
					const state = paletteStates[blockIndexAt(box.region, x + z * box.sx + y * box.sx * box.sz)] ?? 0
					if (state === 0) continue
					const target = ((box.minX + x - minX) >> shift) + row
					if (baseStep === 1) {
						grid[target] = state
					} else {
						// Cells are visited bottom to top, so the highest solid block wins.
						const old = grid[target]
						if (old === 0 || stateFull[state] || !stateFull[old]) grid[target] = state
					}
				}
			}
		}
	}

	// A neighbor hides a face when it is a full block that is opaque, or the
	// same see-through material (glass next to glass, leaves next to leaves).
	// With a coarser grid (step > 1) every cell counts as a full block.
	const occludes = (self: number, neighbor: number, step: number): boolean => {
		if (neighbor === 0) return false
		if (step === 1 && !stateFull[neighbor]) return false
		const neighborLayer = stateRender[neighbor]?.layer
		if (neighborLayer === 0) return true
		return neighborLayer === stateRender[self]?.layer && stateName[neighbor] === stateName[self]
	}

	interface Grid {
		cells: Uint16Array
		dx: number
		dy: number
		dz: number
	}

	// Merges groups of cells of the base grid into one cell (target is the
	// total step). The top-most solid block of a group wins, so roofs and
	// surfaces keep their look.
	const downsample = (targetStep: number): Grid => {
		const factor = targetStep / baseStep
		const nx = Math.ceil(bx / factor)
		const ny = Math.ceil(by / factor)
		const nz = Math.ceil(bz / factor)
		const cells = new Uint16Array(nx * ny * nz)
		for (let cy = 0; cy < ny; cy++) {
			for (let cz = 0; cz < nz; cz++) {
				for (let cx = 0; cx < nx; cx++) {
					let pick = 0
					const yEnd = Math.min(by, (cy + 1) * factor)
					const zEnd = Math.min(bz, (cz + 1) * factor)
					const xEnd = Math.min(bx, (cx + 1) * factor)
					scan: for (let y = yEnd - 1; y >= cy * factor; y--) {
						for (let z = cz * factor; z < zEnd; z++) {
							for (let x = cx * factor; x < xEnd; x++) {
								const state = grid[x + z * bx + y * bx * bz]
								if (state === 0) continue
								if (pick === 0 || (!stateFull[pick] && stateFull[state])) pick = state
								if (stateFull[pick] && stateRender[pick]?.layer === 0) break scan
							}
						}
					}
					cells[cx + cz * nx + cy * nx * nz] = pick
				}
			}
		}
		return { cells, dx: nx, dy: ny, dz: nz }
	}

	const position = new Array<number>(12)
	const uv = new Array<number>(8)

	// Walks over all visible faces. In counting mode (emit = false) it only
	// counts and stops as soon as the limit is passed.
	const walk = (g: Grid, step: number, emit: boolean) => {
		const builders = [new LayerBuilder(), new LayerBuilder(), new LayerBuilder()]
		let faces = 0
		let truncated = false
		const { cells, dx: gx, dy: gy, dz: gz } = g

		outer: for (let y = 0; y < gy; y++) {
			for (let z = 0; z < gz; z++) {
				for (let x = 0; x < gx; x++) {
					const state = cells[x + z * gx + y * gx * gz]
					if (state === 0) continue
					const render = stateRender[state] as BlockRender
					const y0 = step === 1 ? stateY0[state] : 0
					const y1 = step === 1 ? stateY1[state] : 1

					for (const face of FACES) {
						const nx = x + face.dx
						const ny = y + face.dy
						const nz = z + face.dz
						const neighbor =
							nx < 0 || ny < 0 || nz < 0 || nx >= gx || ny >= gy || nz >= gz
								? 0
								: cells[nx + nz * gx + ny * gx * gz]

						// The top of a slab is open to the air even below another block.
						let visible: boolean
						if (face.dy === 1 && y1 < 1) visible = true
						else if (face.dy === -1 && y0 > 0) visible = true
						else visible = !occludes(state, neighbor, step)
						if (!visible) continue

						if (faces >= maxFaces) {
							truncated = true
							break outer
						}
						faces++
						if (!emit) continue

						const rect = face.kind === 'top' ? render.top : face.kind === 'bottom' ? render.bottom : render.side
						const height = rect.v1 - rect.v0
						for (let i = 0; i < 4; i++) {
							const corner = face.corners[i]
							const cornerY = corner[1] ? y1 : y0
							position[i * 3] = (x + corner[0]) * step
							position[i * 3 + 1] = (y + cornerY) * step
							position[i * 3 + 2] = (z + corner[2]) * step
							const [u, v] = UV_PATTERN[i]
							uv[i * 2] = u ? rect.u1 : rect.u0
							// Sides show only the part of the texture that the block covers.
							uv[i * 2 + 1] = face.kind === 'side' ? rect.v0 + cornerY * height : v ? rect.v1 : rect.v0
						}

						const tint = render.tint
						const shade = face.shade
						builders[render.layer].quad(
							position,
							uv,
							Math.round((tint ? tint[0] : 255) * shade),
							Math.round((tint ? tint[1] : 255) * shade),
							Math.round((tint ? tint[2] : 255) * shade),
						)
					}
				}
			}
		}
		return { faces, truncated, builders }
	}

	// Show the whole building: when it has too many faces for full detail, the
	// grid is made coarser (2x, 4x, 8x, 16x) instead of cutting parts off.
	const full: Grid = { cells: grid, dx: bx, dy: by, dz: bz }
	let step = baseStep
	let chosen = full
	while (step < MAX_STEP && walk(chosen, step, false).truncated) {
		step *= 2
		chosen = downsample(step)
	}

	const output = walk(chosen, step, true)
	if (output.faces === 0) return { type: 'empty' }
	return {
		type: 'done',
		result: {
			layers: [output.builders[0].finish(), output.builders[1].finish(), output.builders[2].finish()],
			size: { x: dx, y: dy, z: dz },
			truncated: output.truncated,
			step,
		},
	}
}

self.onmessage = (event: MessageEvent<MeshRequest>) => {
	try {
		const output = build(event.data)
		if (output.type === 'done') {
			const transfer: Transferable[] = []
			for (const layer of output.result.layers) {
				transfer.push(layer.positions.buffer, layer.uvs.buffer, layer.colors.buffer, layer.indices.buffer)
			}
			;(self as unknown as Worker).postMessage(output, transfer)
		} else {
			;(self as unknown as Worker).postMessage(output)
		}
	} catch (error) {
		;(self as unknown as Worker).postMessage({ type: 'failed', message: String(error) })
	}
}
