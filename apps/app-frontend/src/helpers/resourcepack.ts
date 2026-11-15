import { invoke } from '@tauri-apps/api/core'

import { list as listInstances } from '@/helpers/instance'
import { get_game_versions } from '@/helpers/metadata'

export type ResourcePackMetadata = {
	id: string
	name: string
	description: string
	pack_format: number
	game_version: string
	/** null = only game_version, "*" = this version and newer, otherwise the newest version */
	max_game_version?: string | null
	icon_path?: string | null
	created: string
	modified: string
	overridden_assets: string[]
}

export type VanillaAssetKind = 'texture' | 'sound' | 'font' | 'other'

export type VanillaAsset = {
	path: string
	kind: VanillaAssetKind
}

export async function list_resourcepacks(): Promise<ResourcePackMetadata[]> {
	return await invoke('plugin:resourcepack|resourcepack_list')
}

export async function get_resourcepack(id: string): Promise<ResourcePackMetadata | null> {
	return await invoke('plugin:resourcepack|resourcepack_get', { id })
}

export async function create_resourcepack(
	name: string,
	description: string,
	gameVersion: string,
	maxGameVersion: string | null = null,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_create', {
		name,
		description,
		gameVersion,
		maxGameVersion,
	})
}

export async function edit_resourcepack(
	id: string,
	name?: string,
	description?: string,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_edit', { id, name, description })
}

export async function delete_resourcepack(id: string): Promise<void> {
	return await invoke('plugin:resourcepack|resourcepack_delete', { id })
}

/**
 * Lists every overridable vanilla texture/sound/font for the given game
 * version. The game version must have been launched/downloaded at least
 * once already, otherwise this throws.
 */
export async function list_vanilla_assets(gameVersion: string): Promise<VanillaAsset[]> {
	return await invoke('plugin:resourcepack|resourcepack_list_vanilla_assets', {
		gameVersion,
	})
}

export async function read_vanilla_asset(
	gameVersion: string,
	relativeAssetPath: string,
): Promise<Uint8Array> {
	const bytes = await invoke<ArrayBuffer>('plugin:resourcepack|resourcepack_read_vanilla_asset', {
		gameVersion,
		relativeAssetPath,
	})
	return new Uint8Array(bytes)
}

/**
 * Reads the already-saved override for an asset back out of the pack
 * (not the vanilla original). Used so clicking an already-customized
 * item/block texture shows *its own* current image, not a blank/vanilla
 * picker.
 */
export async function read_resourcepack_asset(
	packId: string,
	relativeAssetPath: string,
): Promise<Uint8Array> {
	const bytes = await invoke<ArrayBuffer>('plugin:resourcepack|resourcepack_read_asset', {
		packId,
		relativeAssetPath,
	})
	return new Uint8Array(bytes)
}

export async function set_resourcepack_texture(
	packId: string,
	relativeAssetPath: string,
	pngBytes: Uint8Array,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_set_texture', {
		packId,
		relativeAssetPath,
		pngBytes: Array.from(pngBytes),
	})
}

export async function set_resourcepack_sound(
	packId: string,
	relativeAssetPath: string,
	oggBytes: Uint8Array,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_set_sound', {
		packId,
		relativeAssetPath,
		oggBytes: Array.from(oggBytes),
	})
}

export async function set_resourcepack_icon(
	packId: string,
	pngBytes: Uint8Array,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_set_icon', {
		packId,
		pngBytes: Array.from(pngBytes),
	})
}

export async function remove_resourcepack_asset(
	packId: string,
	relativeAssetPath: string,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_remove_asset', {
		packId,
		relativeAssetPath,
	})
}

export async function deploy_resourcepack_to_instance(
	packId: string,
	instanceId: string,
): Promise<void> {
	return await invoke('plugin:resourcepack|resourcepack_deploy_to_instance', {
		packId,
		instanceId,
	})
}

export async function remove_resourcepack_from_instance(
	packId: string,
	instanceId: string,
): Promise<void> {
	return await invoke('plugin:resourcepack|resourcepack_remove_from_instance', {
		packId,
		instanceId,
	})
}

export async function is_resourcepack_deployed_to_instance(
	packId: string,
	instanceId: string,
): Promise<boolean> {
	return await invoke('plugin:resourcepack|resourcepack_is_deployed_to_instance', {
		packId,
		instanceId,
	})
}

export async function duplicate_resourcepack(id: string): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_duplicate', { id })
}

export async function retarget_resourcepack(
	id: string,
	gameVersion: string,
	maxGameVersion: string | null = null,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_retarget', {
		id,
		gameVersion,
		maxGameVersion,
	})
}

export async function export_resourcepack(packId: string, destination: string): Promise<string> {
	return await invoke('plugin:resourcepack|resourcepack_export', { packId, destination })
}

export async function import_resourcepack(
	zipPath: string,
	name: string | null,
	gameVersion: string,
	maxGameVersion: string | null = null,
): Promise<ResourcePackMetadata> {
	return await invoke('plugin:resourcepack|resourcepack_import', {
		zipPath,
		name,
		gameVersion,
		maxGameVersion,
	})
}

/**
 * Minecraft versions a pack can target: all releases from the launcher
 * metadata (newest first). Falls back to the versions of existing instances
 * when the metadata is not reachable (offline).
 */
export async function list_available_game_versions(): Promise<string[]> {
	try {
		const manifest = (await get_game_versions()) as {
			versions: { id: string; type: string }[]
		}
		const releases = manifest.versions.filter((v) => v.type === 'release').map((v) => v.id)
		if (releases.length > 0) return releases
	} catch {
		// fall through to the offline fallback
	}
	const instances = await listInstances()
	const versions = new Set(instances.map((instance) => instance.game_version))
	return [...versions].sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))
}

/**
 * Reads assets out of the mod jars of an instance. Every path looks like
 * `assets/<namespace>/<path>` (`*` as namespace searches all mods). The result
 * has one entry per path, `null` when the asset does not exist.
 */
export async function read_mod_assets(
	instanceId: string,
	paths: string[],
): Promise<(Uint8Array | null)[]> {
	const buffer = await invoke<ArrayBuffer>('plugin:resourcepack|resourcepack_read_mod_assets', {
		instanceId,
		paths,
	})
	const bytes = new Uint8Array(buffer)
	const view = new DataView(buffer)
	const result: (Uint8Array | null)[] = []
	let offset = 0
	for (let i = 0; i < paths.length; i++) {
		if (offset + 4 > bytes.length) {
			result.push(null)
			continue
		}
		const length = view.getUint32(offset, true)
		offset += 4
		if (length === 0xffffffff) {
			result.push(null)
			continue
		}
		result.push(bytes.slice(offset, offset + length))
		offset += length
	}
	return result
}
