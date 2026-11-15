import { invoke } from '@tauri-apps/api/core'

export interface SchematicFile {
	name: string
	/** Relative to the instance's schematics folder, with `/` separators */
	path: string
	size: number
	/** Milliseconds since the Unix epoch */
	modified: number
}

export async function schematics_has_litematica(instanceDir: string): Promise<boolean> {
	return await invoke('plugin:schematics|schematics_has_litematica', { instanceDir })
}

export async function schematics_list_folders(instanceDir: string): Promise<string[]> {
	return await invoke('plugin:schematics|schematics_list_folders', { instanceDir })
}

export async function schematics_list_files(
	instanceDir: string,
	folder: string,
): Promise<SchematicFile[]> {
	return await invoke('plugin:schematics|schematics_list_files', { instanceDir, folder })
}

export async function schematics_read(instanceDir: string, path: string): Promise<Uint8Array> {
	const bytes = await invoke<ArrayBuffer>('plugin:schematics|schematics_read', {
		instanceDir,
		path,
	})
	return new Uint8Array(bytes)
}

export async function schematics_import(
	instanceDir: string,
	folder: string,
	sources: string[],
): Promise<string[]> {
	return await invoke('plugin:schematics|schematics_import', { instanceDir, folder, sources })
}

export async function schematics_create_folder(instanceDir: string, path: string): Promise<void> {
	return await invoke('plugin:schematics|schematics_create_folder', { instanceDir, path })
}

export async function schematics_delete(instanceDir: string, path: string): Promise<void> {
	return await invoke('plugin:schematics|schematics_delete', { instanceDir, path })
}
