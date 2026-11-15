import { invoke } from '@tauri-apps/api/core'

export interface AperionIngameInfo {
	game_version: string
	/** Newest version already on this computer. */
	installed_version: string | null
	/** Newest version published on GitHub. */
	latest_version: string | null
	/** A build exists (locally or on GitHub) for this Minecraft version. */
	available: boolean
}

export async function aperion_ingame_info(gameVersion: string): Promise<AperionIngameInfo> {
	return await invoke('plugin:aperion-ingame|aperion_ingame_info', { gameVersion })
}

export async function aperion_ingame_prefetch(gameVersion: string): Promise<AperionIngameInfo> {
	return await invoke('plugin:aperion-ingame|aperion_ingame_prefetch', { gameVersion })
}
