/** Shared by the preview component, the texture atlas and the mesh worker. */

/** 0 = opaque, 1 = cutout (leaves, glass panes, plants), 2 = translucent (water, glass, ice) */
export type RenderLayer = 0 | 1 | 2

/** A rectangle in the texture atlas, in UV space (v0 = bottom, v1 = top). */
export interface FaceUV {
	u0: number
	v0: number
	u1: number
	v1: number
}

export interface BlockRender {
	top: FaceUV
	side: FaceUV
	bottom: FaceUV
	/** Color multiplied onto the texture (grass, leaves, water) */
	tint: [number, number, number] | null
	layer: RenderLayer
}

export interface MeshLayerData {
	positions: Float32Array
	uvs: Float32Array
	/** RGBA, 8 bit */
	colors: Uint8Array
	indices: Uint32Array
}

export interface MeshResult {
	layers: [MeshLayerData, MeshLayerData, MeshLayerData]
	size: { x: number; y: number; z: number }
	truncated: boolean
	/** 1 = every block is shown; 2, 4, ... = blocks were merged because the schematic is large */
	step: number
}
