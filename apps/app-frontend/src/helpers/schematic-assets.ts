import { read_mod_assets, read_vanilla_asset } from '@/helpers/resourcepack'

/**
 * Finds the textures of blocks the way the game does: blockstate -> model ->
 * texture. Vanilla assets come from the Minecraft jar, everything else from the
 * mod jars of the instance. Whatever cannot be found gives `null`, the caller
 * then draws a placeholder.
 */

export interface AssetSource {
	gameVersion: string
	instanceId?: string
}

export type FaceName = 'top' | 'side' | 'bottom'

export interface TextureHit {
	/** `namespace:path` of the texture */
	texture: string
	bitmap: ImageBitmap
}

const MAX_BATCH = 256

function splitId(id: string): [string, string] {
	const index = id.indexOf(':')
	return index === -1 ? ['minecraft', id] : [id.slice(0, index), id.slice(index + 1)]
}

// ---------- reading ----------

// Reads from the mod jars of the same moment are sent as one request.
const modQueue = new Map<string, { paths: string[]; resolvers: ((bytes: Uint8Array | null) => void)[] }>()

async function flushMods(instanceId: string) {
	const queue = modQueue.get(instanceId)
	modQueue.delete(instanceId)
	if (!queue) return

	for (let start = 0; start < queue.paths.length; start += MAX_BATCH) {
		const paths = queue.paths.slice(start, start + MAX_BATCH)
		const resolvers = queue.resolvers.slice(start, start + MAX_BATCH)
		try {
			const results = await read_mod_assets(instanceId, paths)
			resolvers.forEach((resolve, index) => resolve(results[index] ?? null))
		} catch {
			resolvers.forEach((resolve) => resolve(null))
		}
	}
}

function readModAsset(instanceId: string, assetPath: string): Promise<Uint8Array | null> {
	return new Promise((resolve) => {
		let queue = modQueue.get(instanceId)
		if (!queue) {
			queue = { paths: [], resolvers: [] }
			modQueue.set(instanceId, queue)
			setTimeout(() => void flushMods(instanceId), 0)
		}
		queue.paths.push(assetPath)
		queue.resolvers.push(resolve)
	})
}

// Everything that was read (also what was missing) stays in memory.
const rawCache = new Map<string, Promise<Uint8Array | null>>()

function readAsset(source: AssetSource, namespace: string, path: string): Promise<Uint8Array | null> {
	const key = `${source.gameVersion}|${source.instanceId ?? ''}|${namespace}|${path}`
	let promise = rawCache.get(key)
	if (!promise) {
		if (namespace === 'minecraft') {
			promise = read_vanilla_asset(source.gameVersion, `assets/minecraft/${path}`)
				.then((bytes): Uint8Array | null => bytes)
				.catch(() => null)
		} else if (source.instanceId) {
			promise = readModAsset(source.instanceId, `assets/${namespace}/${path}`)
		} else {
			promise = Promise.resolve(null)
		}
		rawCache.set(key, promise)
	}
	return promise
}

async function readJson<T>(source: AssetSource, namespace: string, path: string): Promise<T | null> {
	const bytes = await readAsset(source, namespace, path)
	if (!bytes) return null
	try {
		return JSON.parse(new TextDecoder().decode(bytes)) as T
	} catch {
		return null
	}
}

// ---------- textures ----------

const textureCache = new Map<string, Promise<TextureHit | null>>()

/**
 * Loads a texture like `create:block/cogwheel`. With `fallback` (the default),
 * a texture that is missing in its own namespace is searched in all other mods
 * and finally in vanilla. Guessed names should not use this, because an
 * unrelated mod could have a file with the same name.
 */
export function loadTexture(
	source: AssetSource,
	textureId: string,
	fallback = true,
): Promise<TextureHit | null> {
	const [namespace, path] = splitId(textureId)
	const key = `${source.gameVersion}|${source.instanceId ?? ''}|${namespace}:${path}|${fallback}`
	let promise = textureCache.get(key)
	if (!promise) {
		promise = (async () => {
			const file = `textures/${path}.png`
			let bytes = await readAsset(source, namespace, file)
			if (!bytes && fallback && source.instanceId) {
				bytes = await readModAsset(source.instanceId, `assets/*/${file}`)
			}
			if (!bytes && fallback && namespace !== 'minecraft') bytes = await readAsset(source, 'minecraft', file)
			if (!bytes) return null
			try {
				const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
				return { texture: `${namespace}:${path}`, bitmap }
			} catch {
				return null
			}
		})()
		textureCache.set(key, promise)
	}
	return promise
}

// ---------- blockstates and models ----------

interface StateModel {
	model?: string
}
interface BlockstateJson {
	variants?: Record<string, StateModel | StateModel[]>
	multipart?: { when?: unknown; apply?: StateModel | StateModel[] }[]
}
interface ModelElement {
	from?: number[]
	to?: number[]
	faces?: Record<string, { texture?: string }>
}
interface ModelJson {
	parent?: string
	textures?: Record<string, string>
	elements?: ModelElement[]
}

function firstModel(value: StateModel | StateModel[] | undefined): string | null {
	const entry = Array.isArray(value) ? value[0] : value
	return typeof entry?.model === 'string' ? entry.model : null
}

async function blockModelId(source: AssetSource, namespace: string, id: string): Promise<string | null> {
	const json = await readJson<BlockstateJson>(source, namespace, `blockstates/${id}.json`)
	if (!json) return null

	let model: string | null = null
	if (json.variants) {
		const keys = Object.keys(json.variants)
		const key = keys.includes('') ? '' : keys[0]
		if (key !== undefined) model = firstModel(json.variants[key])
	} else if (json.multipart?.length) {
		const part = json.multipart.find((entry) => !entry.when) ?? json.multipart[0]
		model = firstModel(part.apply)
	}
	if (!model) return null
	return model.includes(':') ? model : `${namespace}:${model}`
}

function volume(element: ModelElement): number {
	const from = element.from ?? [0, 0, 0]
	const to = element.to ?? [16, 16, 16]
	return Math.abs((to[0] - from[0]) * (to[1] - from[1]) * (to[2] - from[2]))
}

async function modelTextures(
	source: AssetSource,
	modelId: string,
	defaultNamespace: string,
): Promise<Record<FaceName, string> | null> {
	const textures: Record<string, string> = {}
	let elements: ModelElement[] | undefined

	let next: string | undefined = modelId.includes(':') ? modelId : `${defaultNamespace}:${modelId}`
	for (let depth = 0; next && depth < 8; depth++) {
		const [namespace, path] = splitId(next)
		if (path.startsWith('builtin/')) break
		const model: ModelJson | null = await readJson<ModelJson>(source, namespace, `models/${path}.json`)
		if (!model) break
		for (const [name, value] of Object.entries(model.textures ?? {})) {
			if (!(name in textures) && typeof value === 'string') textures[name] = value
		}
		if (!elements && Array.isArray(model.elements) && model.elements.length > 0) elements = model.elements
		next = model.parent
	}

	const resolveReference = (reference: string | undefined): string | null => {
		let current = reference
		for (let i = 0; i < 8 && current; i++) {
			if (!current.startsWith('#')) return current
			current = textures[current.slice(1)]
		}
		return null
	}
	// The texture of the biggest element that has a face in one of the directions
	const fromElements = (directions: string[]): string | null => {
		let best: string | null = null
		let bestVolume = -1
		for (const element of elements ?? []) {
			for (const direction of directions) {
				const texture = resolveReference(element.faces?.[direction]?.texture)
				if (!texture) continue
				const size = volume(element)
				if (size > bestVolume) {
					best = texture
					bestVolume = size
				}
				break
			}
		}
		return best
	}
	const fromVariables = (names: string[]): string | null => {
		for (const name of names) {
			const texture = resolveReference(`#${name}`)
			if (texture) return texture
		}
		return null
	}

	const top = fromElements(['up']) ?? fromVariables(['top', 'up', 'end', 'all', 'particle'])
	const side =
		fromElements(['north', 'south', 'east', 'west']) ??
		fromVariables(['side', 'all', 'north', 'front', 'particle'])
	const bottom = fromElements(['down']) ?? fromVariables(['bottom', 'down', 'end', 'all', 'particle'])
	const any = top ?? side ?? bottom
	if (!any) return null
	return { top: top ?? any, side: side ?? any, bottom: bottom ?? top ?? any }
}

const blockCache = new Map<string, Promise<Record<FaceName, string> | null>>()

/** The texture ids (`namespace:path`) of the top, side and bottom of a block. */
export function resolveBlockTextures(
	source: AssetSource,
	blockName: string,
): Promise<Record<FaceName, string> | null> {
	const key = `${source.gameVersion}|${source.instanceId ?? ''}|${blockName}`
	let promise = blockCache.get(key)
	if (!promise) {
		promise = (async () => {
			try {
				const [namespace, id] = splitId(blockName)
				const model = await blockModelId(source, namespace, id)
				if (!model) return null
				return await modelTextures(source, model, namespace)
			} catch {
				return null
			}
		})()
		blockCache.set(key, promise)
	}
	return promise
}
