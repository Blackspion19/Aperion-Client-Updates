/**
 * Reader for Litematica's `.litematic` files: a gzip compressed NBT file.
 * Only what the schematic manager needs is read: the metadata for the info
 * panel and the block data for the 3D preview.
 */

// ---------- NBT ----------

type NbtValue =
	| number
	| bigint
	| string
	| Uint8Array
	| Int32Array
	| Uint32Array // long arrays, stored as 32-bit words (low, high, low, high, ...)
	| NbtValue[]
	| { [key: string]: NbtValue }

class NbtReader {
	private view: DataView
	private offset = 0
	private decoder = new TextDecoder('utf-8')

	constructor(private bytes: Uint8Array) {
		this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
	}

	private string(): string {
		const length = this.view.getUint16(this.offset)
		this.offset += 2
		const text = this.decoder.decode(this.bytes.subarray(this.offset, this.offset + length))
		this.offset += length
		return text
	}

	private payload(type: number): NbtValue {
		switch (type) {
			case 1: {
				const value = this.view.getInt8(this.offset)
				this.offset += 1
				return value
			}
			case 2: {
				const value = this.view.getInt16(this.offset)
				this.offset += 2
				return value
			}
			case 3: {
				const value = this.view.getInt32(this.offset)
				this.offset += 4
				return value
			}
			case 4: {
				const value = this.view.getBigInt64(this.offset)
				this.offset += 8
				return value
			}
			case 5: {
				const value = this.view.getFloat32(this.offset)
				this.offset += 4
				return value
			}
			case 6: {
				const value = this.view.getFloat64(this.offset)
				this.offset += 8
				return value
			}
			case 7: {
				const length = this.view.getInt32(this.offset)
				this.offset += 4
				const value = this.bytes.slice(this.offset, this.offset + length)
				this.offset += length
				return value
			}
			case 8:
				return this.string()
			case 9: {
				const itemType = this.view.getUint8(this.offset)
				const length = this.view.getInt32(this.offset + 1)
				this.offset += 5
				const list: NbtValue[] = []
				for (let i = 0; i < length; i++) list.push(this.payload(itemType))
				return list
			}
			case 10: {
				const compound: { [key: string]: NbtValue } = {}
				for (;;) {
					const childType = this.view.getUint8(this.offset)
					this.offset += 1
					if (childType === 0) break
					const name = this.string()
					compound[name] = this.payload(childType)
				}
				return compound
			}
			case 11: {
				const length = this.view.getInt32(this.offset)
				this.offset += 4
				const value = new Int32Array(length)
				for (let i = 0; i < length; i++) value[i] = this.view.getInt32(this.offset + i * 4)
				this.offset += length * 4
				return value
			}
			case 12: {
				// A long is two big-endian words: high first. Stored as low, high.
				const length = this.view.getInt32(this.offset)
				this.offset += 4
				const words = new Uint32Array(length * 2)
				for (let i = 0; i < length; i++) {
					words[i * 2 + 1] = this.view.getUint32(this.offset + i * 8)
					words[i * 2] = this.view.getUint32(this.offset + i * 8 + 4)
				}
				this.offset += length * 8
				return words
			}
			default:
				throw new Error(`Unknown NBT tag ${type}`)
		}
	}

	readRoot(): { [key: string]: NbtValue } {
		const type = this.view.getUint8(this.offset)
		this.offset += 1
		if (type !== 10) throw new Error('Not an NBT compound')
		this.string() // root name
		return this.payload(10) as { [key: string]: NbtValue }
	}
}

async function gunzip(bytes: Uint8Array): Promise<Uint8Array> {
	// Some tools write the file without compression.
	if (bytes.length < 2 || bytes[0] !== 0x1f || bytes[1] !== 0x8b) return bytes
	const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new DecompressionStream('gzip'))
	return new Uint8Array(await new Response(stream).arrayBuffer())
}

// ---------- data model ----------

export interface Vec3 {
	x: number
	y: number
	z: number
}

export type SchematicFormat = 'litematic' | 'sponge' | 'structure'

export interface SchematicInfo {
	format: SchematicFormat
	name: string
	author: string
	description: string
	timeCreated: number
	timeModified: number
	regionCount: number
	totalVolume: number
	totalBlocks: number
	enclosingSize: Vec3
	version: number
	dataVersion: number
	minecraftVersion: string | null
}

export interface BlockState {
	name: string
	properties: Record<string, string>
}

export interface SchematicRegion {
	name: string
	position: Vec3
	size: Vec3
	palette: BlockState[]
	/** Packed block indices (32-bit words), see `blockIndexAt` */
	words: Uint32Array
	bits: number
	/** Formats that do not pack their blocks give the palette index of every cell directly */
	indices?: Uint32Array
}

export interface ParsedSchematic {
	info: SchematicInfo
	regions: SchematicRegion[]
}

// Minecraft data versions of released versions (data version -> name).
const DATA_VERSIONS: Record<number, string> = {
	1343: '1.12.2',
	1631: '1.13.2',
	1976: '1.14.4',
	2230: '1.15.2',
	2566: '1.16',
	2567: '1.16.1',
	2578: '1.16.2',
	2580: '1.16.3',
	2584: '1.16.4',
	2586: '1.16.5',
	2724: '1.17',
	2730: '1.17.1',
	2860: '1.18',
	2865: '1.18.1',
	2975: '1.18.2',
	3105: '1.19',
	3117: '1.19.1',
	3120: '1.19.2',
	3218: '1.19.3',
	3337: '1.19.4',
	3463: '1.20',
	3465: '1.20.1',
	3578: '1.20.2',
	3698: '1.20.3',
	3700: '1.20.4',
	3837: '1.20.5',
	3839: '1.20.6',
	3953: '1.21',
	3955: '1.21.1',
	4080: '1.21.2',
	4082: '1.21.3',
	4189: '1.21.4',
	4325: '1.21.5',
	4435: '1.21.6',
	4438: '1.21.7',
	4440: '1.21.8',
	4554: '1.21.9',
	4556: '1.21.10',
	4671: '1.21.11',
}

function asObject(value: NbtValue | undefined): { [key: string]: NbtValue } {
	return value && typeof value === 'object' && !Array.isArray(value) && !ArrayBuffer.isView(value)
		? (value as { [key: string]: NbtValue })
		: {}
}

function asNumber(value: NbtValue | undefined): number {
	if (typeof value === 'bigint') return Number(value)
	return typeof value === 'number' ? value : 0
}

function asString(value: NbtValue | undefined): string {
	return typeof value === 'string' ? value : ''
}

function asVec3(value: NbtValue | undefined): Vec3 {
	const object = asObject(value)
	return { x: asNumber(object.x), y: asNumber(object.y), z: asNumber(object.z) }
}

export async function parseLitematic(fileBytes: Uint8Array): Promise<ParsedSchematic> {
	const root = new NbtReader(await gunzip(fileBytes)).readRoot()
	const metadata = asObject(root.Metadata)
	const dataVersion = asNumber(root.MinecraftDataVersion)

	const info: SchematicInfo = {
		format: 'litematic',
		name: asString(metadata.Name),
		author: asString(metadata.Author),
		description: asString(metadata.Description),
		timeCreated: asNumber(metadata.TimeCreated),
		timeModified: asNumber(metadata.TimeModified),
		regionCount: asNumber(metadata.RegionCount),
		totalVolume: asNumber(metadata.TotalVolume),
		totalBlocks: asNumber(metadata.TotalBlocks),
		enclosingSize: asVec3(metadata.EnclosingSize),
		version: asNumber(root.Version),
		dataVersion,
		minecraftVersion: DATA_VERSIONS[dataVersion] ?? null,
	}

	const regions: SchematicRegion[] = []
	for (const [name, value] of Object.entries(asObject(root.Regions))) {
		const region = asObject(value)
		const palette: BlockState[] = ((region.BlockStatePalette as NbtValue[]) ?? []).map((entry) => {
			const compound = asObject(entry)
			const properties: Record<string, string> = {}
			for (const [key, property] of Object.entries(asObject(compound.Properties))) {
				properties[key] = asString(property)
			}
			return { name: asString(compound.Name), properties }
		})
		const words = region.BlockStates instanceof Uint32Array ? region.BlockStates : new Uint32Array(0)
		regions.push({
			name,
			position: asVec3(region.Position),
			size: asVec3(region.Size),
			palette,
			words,
			bits: Math.max(2, Math.ceil(Math.log2(Math.max(palette.length, 1)))),
		})
	}

	return { info, regions }
}

/**
 * Palette index of the block at the given (non-negative) position inside a
 * region. Litematica packs the indices tightly into a long array, and an
 * entry may reach over from one long into the next.
 */
export function blockIndexAt(region: SchematicRegion, index: number): number {
	if (region.indices) return region.indices[index] ?? 0
	const { words, bits } = region
	const start = index * bits
	const word = start >>> 5
	const offset = start & 31
	let value = words[word] >>> offset
	if (offset + bits > 32) value |= words[word + 1] << (32 - offset)
	const mask = bits >= 32 ? 0xffffffff : (1 << bits) - 1
	return (value & mask) >>> 0
}

export function isAirLike(name: string): boolean {
	return (
		name === 'minecraft:air' ||
		name === 'minecraft:cave_air' ||
		name === 'minecraft:void_air' ||
		name === 'minecraft:structure_void' ||
		name === 'minecraft:light'
	)
}

// ---------- Sponge schematics (.schem) and vanilla structures (.nbt) ----------

const AIR: BlockState = { name: 'minecraft:air', properties: {} }

/** "minecraft:oak_stairs[facing=north,half=bottom]" -> name and properties */
function parseBlockString(key: string): BlockState {
	const bracket = key.indexOf('[')
	if (bracket === -1) return { name: key, properties: {} }
	const properties: Record<string, string> = {}
	const end = key.lastIndexOf(']')
	for (const pair of key.slice(bracket + 1, end === -1 ? undefined : end).split(',')) {
		const [name, value] = pair.split('=')
		if (name) properties[name] = value ?? ''
	}
	return { name: key.slice(0, bracket), properties }
}

function countBlocks(indices: Uint32Array, palette: BlockState[]): number {
	const solid = palette.map((state) => !isAirLike(state.name))
	let count = 0
	for (let i = 0; i < indices.length; i++) if (solid[indices[i]]) count++
	return count
}

function parseSponge(root: { [key: string]: NbtValue }, fileName: string): ParsedSchematic {
	// Version 3 wraps everything in a "Schematic" compound
	const data = root.Schematic ? asObject(root.Schematic) : root
	const version = asNumber(data.Version)
	const metadata = asObject(data.Metadata)
	const width = asNumber(data.Width)
	const height = asNumber(data.Height)
	const length = asNumber(data.Length)

	const blocks = asObject(data.Blocks)
	const paletteTag = asObject(data.Blocks ? blocks.Palette : data.Palette)
	const blockData = (data.Blocks ? blocks.Data : data.BlockData) as NbtValue

	const palette: BlockState[] = []
	for (const [key, index] of Object.entries(paletteTag)) {
		palette[asNumber(index)] = parseBlockString(key)
	}
	for (let i = 0; i < palette.length; i++) palette[i] ??= AIR

	const total = width * height * length
	const indices = new Uint32Array(total)
	if (blockData instanceof Uint8Array) {
		// Every block is a variable-length integer (7 bits per byte).
		let position = 0
		for (let i = 0; i < total && position < blockData.length; i++) {
			let value = 0
			let shift = 0
			for (;;) {
				const byte = blockData[position++]
				value |= (byte & 0x7f) << shift
				if ((byte & 0x80) === 0 || position >= blockData.length) break
				shift += 7
			}
			indices[i] = value >>> 0
		}
	}

	const dataVersion = asNumber(data.DataVersion)
	const name = asString(metadata.Name) || fileName.replace(/\.[^.]+$/, '')
	return {
		info: {
			format: 'sponge',
			name,
			author: asString(metadata.Author),
			description: '',
			timeCreated: asNumber(metadata.Date),
			timeModified: 0,
			regionCount: 1,
			totalVolume: total,
			totalBlocks: countBlocks(indices, palette),
			enclosingSize: { x: width, y: height, z: length },
			version,
			dataVersion,
			minecraftVersion: DATA_VERSIONS[dataVersion] ?? null,
		},
		regions: [
			{
				name,
				position: { x: 0, y: 0, z: 0 },
				size: { x: width, y: height, z: length },
				palette,
				words: new Uint32Array(0),
				bits: 0,
				indices,
			},
		],
	}
}

function parseStructure(root: { [key: string]: NbtValue }, fileName: string): ParsedSchematic {
	const sizeList = Array.isArray(root.size) ? root.size.map(asNumber) : [0, 0, 0]
	const [sx, sy, sz] = sizeList

	// Newer files can hold several palettes (shipwrecks); the first one is used.
	const paletteList = Array.isArray(root.palette)
		? root.palette
		: Array.isArray(root.palettes) && Array.isArray(root.palettes[0])
			? (root.palettes[0] as NbtValue[])
			: []
	// Index 0 is air: cells that are not listed in the file are empty.
	const palette: BlockState[] = [AIR]
	for (const entry of paletteList) {
		const compound = asObject(entry)
		const properties: Record<string, string> = {}
		for (const [key, value] of Object.entries(asObject(compound.Properties))) {
			properties[key] = asString(value)
		}
		palette.push({ name: asString(compound.Name), properties })
	}

	const indices = new Uint32Array(Math.max(0, sx * sy * sz))
	for (const entry of Array.isArray(root.blocks) ? root.blocks : []) {
		const block = asObject(entry)
		const position = Array.isArray(block.pos) ? block.pos.map(asNumber) : []
		if (position.length < 3) continue
		const [x, y, z] = position
		if (x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz) continue
		indices[x + z * sx + y * sx * sz] = asNumber(block.state) + 1
	}

	const dataVersion = asNumber(root.DataVersion)
	const name = fileName.replace(/\.[^.]+$/, '')
	return {
		info: {
			format: 'structure',
			name,
			author: '',
			description: '',
			timeCreated: 0,
			timeModified: 0,
			regionCount: 1,
			totalVolume: sx * sy * sz,
			totalBlocks: countBlocks(indices, palette),
			enclosingSize: { x: sx, y: sy, z: sz },
			version: 0,
			dataVersion,
			minecraftVersion: DATA_VERSIONS[dataVersion] ?? null,
		},
		regions: [
			{
				name,
				position: { x: 0, y: 0, z: 0 },
				size: { x: sx, y: sy, z: sz },
				palette,
				words: new Uint32Array(0),
				bits: 0,
				indices,
			},
		],
	}
}

/** Formats whose details and preview can be read. */
export function isReadableSchematic(fileName: string): boolean {
	return /\.(litematic|schem|nbt)$/i.test(fileName)
}

/** Reads a .litematic, .schem (Sponge) or .nbt (vanilla structure) file. */
export async function parseSchematicFile(
	fileBytes: Uint8Array,
	fileName: string,
): Promise<ParsedSchematic> {
	if (/\.litematic$/i.test(fileName)) return parseLitematic(fileBytes)

	const root = new NbtReader(await gunzip(fileBytes)).readRoot()
	if (/\.schem$/i.test(fileName)) return parseSponge(root, fileName)
	if (/\.nbt$/i.test(fileName)) {
		// Some mods save .nbt files in the Sponge layout.
		if (root.BlockData || root.Blocks || root.Schematic) return parseSponge(root, fileName)
		return parseStructure(root, fileName)
	}
	throw new Error('Unsupported schematic format')
}
