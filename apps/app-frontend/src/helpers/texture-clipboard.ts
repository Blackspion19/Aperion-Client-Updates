import { shallowRef } from 'vue'

/**
 * A texture that was copied in the texture painter, kept inside the app so it
 * can be pasted into any other texture (of the same or another pack).
 */
export interface CopiedTexture {
	/** The asset the texture was copied from, e.g. assets/minecraft/textures/block/stone.png */
	source: string
	png: Uint8Array
	width: number
	height: number
}

export const copiedTexture = shallowRef<CopiedTexture | null>(null)
