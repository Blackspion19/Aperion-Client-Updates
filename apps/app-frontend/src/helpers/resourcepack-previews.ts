import { read_resourcepack_asset, read_vanilla_asset } from '@/helpers/resourcepack'

/**
 * Thumbnail loading for the resource pack editor.
 *
 * - at most a few reads run at the same time, so the UI never gets flooded
 * - every image is loaded once and kept as an object URL
 * - the cache lives outside of Vue, so loading a thumbnail never re-renders
 *   the whole list
 */

const MAX_PARALLEL = 4
let running = 0
const waiting: (() => void)[] = []

function schedule<T>(task: () => Promise<T>): Promise<T> {
	return new Promise<T>((resolve, reject) => {
		const start = () => {
			running++
			task()
				.then(resolve, reject)
				.finally(() => {
					running--
					waiting.shift()?.()
				})
		}
		if (running < MAX_PARALLEL) start()
		else waiting.push(start)
	})
}

const urls = new Map<string, string>()
const inFlight = new Map<string, Promise<string | null>>()

async function load(key: string, read: () => Promise<Uint8Array>): Promise<string | null> {
	const cached = urls.get(key)
	if (cached) return cached
	const running = inFlight.get(key)
	if (running) return running

	const promise = schedule(async () => {
		try {
			const bytes = await read()
			const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/png' }))
			urls.set(key, url)
			return url
		} catch {
			return null
		} finally {
			inFlight.delete(key)
		}
	})
	inFlight.set(key, promise)
	return promise
}

export function vanillaPreview(gameVersion: string, path: string): Promise<string | null> {
	return load(`v:${gameVersion}:${path}`, () => read_vanilla_asset(gameVersion, path))
}

/** `revision` changes whenever the pack's own copy of the asset was saved. */
export function packPreview(
	packId: string,
	path: string,
	revision: number,
): Promise<string | null> {
	return load(`p:${packId}:${revision}:${path}`, () => read_resourcepack_asset(packId, path))
}

export function clearPreviews(prefix?: string) {
	for (const [key, url] of urls) {
		if (!prefix || key.startsWith(prefix)) {
			URL.revokeObjectURL(url)
			urls.delete(key)
		}
	}
}
