import { type BlockRender, type FaceUV, type RenderLayer } from '@/helpers/schematic-types'
import {
	type AssetSource,
	loadTexture,
	resolveBlockTextures,
} from '@/helpers/schematic-assets'

/**
 * One texture atlas for all blocks of a schematic. Every block face looks up
 * its rectangle in this single image, so the whole preview needs only a few
 * draw calls and one texture.
 */

const CELL = 16
const MAX_ATLAS = 4096

export interface Atlas {
	canvas: HTMLCanvasElement
	blocks: Record<string, BlockRender>
}

type Face = 'top' | 'side' | 'bottom'

function shortName(name: string): string {
	return name.replace(/^[^:]+:/, '')
}

function namespaceOf(name: string): string {
	const index = name.indexOf(':')
	return index === -1 ? 'minecraft' : name.slice(0, index)
}

function tintFor(name: string): [number, number, number] | null {
	const id = shortName(name)
	if (id === 'water') return [63, 118, 228]
	if (id.endsWith('_leaves') || id === 'vine' || id === 'lily_pad') return [89, 174, 48]
	if (id === 'grass_block' || id === 'grass' || id === 'short_grass' || id === 'tall_grass' || id === 'fern')
		return [124, 189, 107]
	return null
}

function hashColor(name: string): [number, number, number] {
	let hash = 0
	for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0
	const h = (Math.abs(hash) % 360) / 360
	// HSL (s 0.35, l 0.5) -> RGB
	const s = 0.35
	const l = 0.5
	const q = l + s * (1 - l)
	const p = 2 * l - q
	const channel = (t: number) => {
		let x = t
		if (x < 0) x += 1
		if (x > 1) x -= 1
		if (x < 1 / 6) return p + (q - p) * 6 * x
		if (x < 1 / 2) return q
		if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6
		return p
	}
	return [channel(h + 1 / 3) * 255, channel(h) * 255, channel(h - 1 / 3) * 255].map(Math.round) as [
		number,
		number,
		number,
	]
}

function candidates(name: string, face: Face): string[] {
	const id = shortName(name)
	const base = id.replace(
		/_(stairs|slab|wall|fence_gate|fence|button|pressure_plate|trapdoor|sign|wall_sign)$/,
		'',
	)
	const roots = [...new Set([id, base, `${base}_planks`, `${base}s`, `${base}_block`])]
	const suffixes =
		face === 'top' ? ['_top', '', '_side'] : face === 'bottom' ? ['_bottom', '_top', ''] : ['_side', '', '_front', '_top']
	const list: string[] = []
	for (const root of roots) for (const suffix of suffixes) list.push(`${root}${suffix}`)
	return list.slice(0, 10)
}

// ---------- textures ----------

async function fromHeuristics(
	source: AssetSource,
	name: string,
	face: Face,
): Promise<{ texture: string; bitmap: ImageBitmap } | null> {
	const namespace = namespaceOf(name)
	for (const texture of candidates(name, face)) {
		const hit = await loadTexture(source, `${namespace}:block/${texture}`, false)
		if (hit) return hit
	}
	return null
}

async function fromModel(
	source: AssetSource,
	name: string,
	face: Face,
): Promise<{ texture: string; bitmap: ImageBitmap } | null> {
	const textures = await resolveBlockTextures(source, name)
	const reference = textures?.[face]
	return reference ? await loadTexture(source, reference) : null
}

/**
 * Vanilla blocks are found by their name first and by their model second, blocks
 * from mods the other way around. Without a texture the caller uses a
 * placeholder color.
 */
async function resolveFace(
	source: AssetSource,
	name: string,
	face: Face,
): Promise<{ texture: string; bitmap: ImageBitmap } | null> {
	try {
		const vanilla = namespaceOf(name) === 'minecraft'
		const order = vanilla ? [fromHeuristics, fromModel] : [fromModel, fromHeuristics]
		for (const lookup of order) {
			const hit = await lookup(source, name, face)
			if (hit) return hit
		}
	} catch (error) {
		console.warn(`Schematic preview: no texture for ${name}`, error)
	}
	return null
}

/** True when the first frame of the texture has see-through pixels. */
function hasTransparency(bitmap: ImageBitmap): boolean {
	const canvas = document.createElement('canvas')
	canvas.width = CELL
	canvas.height = CELL
	const context = canvas.getContext('2d', { willReadFrequently: true })
	if (!context) return false
	context.imageSmoothingEnabled = false
	context.drawImage(bitmap, 0, 0, bitmap.width, Math.min(bitmap.height, bitmap.width), 0, 0, CELL, CELL)
	const data = context.getImageData(0, 0, CELL, CELL).data
	for (let i = 3; i < data.length; i += 4) if (data[i] < 200) return true
	return false
}

async function pool<T>(items: T[], limit: number, task: (item: T) => Promise<void>) {
	let next = 0
	await Promise.all(
		Array.from({ length: Math.min(limit, items.length) }, async () => {
			while (next < items.length) await task(items[next++])
		}),
	)
}

const atlasCache = new Map<string, Atlas>()

export async function buildAtlas(
	gameVersion: string,
	names: string[],
	instanceId?: string,
): Promise<Atlas> {
	const source: AssetSource = { gameVersion, instanceId }
	const cacheKey = `${gameVersion}|${instanceId ?? ''}|${[...names].sort().join(',')}`
	const cached = atlasCache.get(cacheKey)
	if (cached) return cached

	type Pending = {
		name: string
		face: Face
		cell: string // texture file, or "color:r,g,b"
		bitmap: ImageBitmap | null
		color: [number, number, number]
	}
	const resolved = new Map<string, Pending>() // `${name}|${face}`
	const tasks: { name: string; face: Face }[] = []
	for (const name of names) for (const face of ['top', 'side', 'bottom'] as const) tasks.push({ name, face })

	await pool(tasks, 8, async ({ name, face }) => {
		const id = shortName(name)
		const flat = namespaceOf(name) === 'minecraft' && (id === 'water' || id === 'lava')
		const found = flat ? null : await resolveFace(source, name, face)
		const color = id === 'water' ? ([63, 118, 228] as [number, number, number]) : hashColor(name)
		resolved.set(`${name}|${face}`, {
			name,
			face,
			cell: found ? found.texture : `color:${color.join(',')}`,
			bitmap: found ? found.bitmap : null,
			color,
		})
	})

	// Every different texture (or flat color) gets one cell in the atlas.
	const cells = new Map<string, { index: number; pending: Pending }>()
	for (const pending of resolved.values()) {
		if (!cells.has(pending.cell)) cells.set(pending.cell, { index: cells.size, pending })
	}
	const count = Math.max(1, cells.size)
	const columns = Math.min(Math.floor(MAX_ATLAS / CELL), Math.ceil(Math.sqrt(count)))
	const rows = Math.ceil(count / columns)
	const width = columns * CELL
	const height = rows * CELL

	const canvas = document.createElement('canvas')
	canvas.width = width
	canvas.height = height
	const context = canvas.getContext('2d')
	if (!context) throw new Error('No canvas')
	context.imageSmoothingEnabled = false
	for (const { index, pending } of cells.values()) {
		const x = (index % columns) * CELL
		const y = Math.floor(index / columns) * CELL
		if (pending.bitmap) {
			// Animated textures are a tall strip: the first frame is enough.
			const frame = Math.min(pending.bitmap.height, pending.bitmap.width)
			context.drawImage(pending.bitmap, 0, 0, pending.bitmap.width, frame, x, y, CELL, CELL)
		} else {
			context.fillStyle = `rgb(${pending.color.join(',')})`
			context.fillRect(x, y, CELL, CELL)
		}
	}

	// Half a texel is cut off every rectangle so neighbors never bleed in.
	const insetU = 0.5 / width
	const insetV = 0.5 / height
	const uvOf = (cell: string): FaceUV => {
		const index = cells.get(cell)?.index ?? 0
		const x = index % columns
		const y = Math.floor(index / columns)
		return {
			u0: (x * CELL) / width + insetU,
			u1: ((x + 1) * CELL) / width - insetU,
			v1: 1 - (y * CELL) / height - insetV,
			v0: 1 - ((y + 1) * CELL) / height + insetV,
		}
	}

	const blocks: Record<string, BlockRender> = {}
	for (const name of names) {
		const id = shortName(name)
		const top = resolved.get(`${name}|top`)
		const side = resolved.get(`${name}|side`)
		const bottom = resolved.get(`${name}|bottom`)
		if (!top || !side || !bottom) continue
		let layer: RenderLayer = 0
		if (id === 'water' || /glass|ice/.test(id)) layer = 2
		else if (side.bitmap && hasTransparency(side.bitmap)) layer = 1
		else if (top.bitmap && hasTransparency(top.bitmap)) layer = 1
		blocks[name] = {
			top: uvOf(top.cell),
			side: uvOf(side.cell),
			bottom: uvOf(bottom.cell),
			tint: tintFor(name),
			layer,
		}
	}

	const atlas: Atlas = { canvas, blocks }
	atlasCache.set(cacheKey, atlas)
	if (atlasCache.size > 6) atlasCache.delete(atlasCache.keys().next().value as string)
	return atlas
}
