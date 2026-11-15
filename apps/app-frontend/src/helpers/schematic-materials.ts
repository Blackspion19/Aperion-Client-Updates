import { blockIndexAt, type BlockState, isAirLike, type ParsedSchematic } from '@/helpers/litematic'

/** One kind of item that is needed to build a schematic. */
export interface MaterialEntry {
	/** Item id, for example "minecraft:oak_planks" */
	id: string
	name: string
	count: number
}

// Blocks that cannot be held as an item, or only exist as part of something else.
const NO_ITEM = new Set([
	'water',
	'lava',
	'bubble_column',
	'fire',
	'soul_fire',
	'piston_head',
	'moving_piston',
	'end_portal',
	'end_gateway',
	'nether_portal',
	'frosted_ice',
	'powder_snow',
	'light',
	'barrier',
	'structure_void',
	'pitcher_crop',
])

// Blocks that drop or are placed as a different item.
const ITEM_FOR_BLOCK: Record<string, string> = {
	redstone_wire: 'redstone',
	tripwire: 'string',
	wheat: 'wheat_seeds',
	carrots: 'carrot',
	potatoes: 'potato',
	beetroots: 'beetroot_seeds',
	melon_stem: 'melon_seeds',
	attached_melon_stem: 'melon_seeds',
	pumpkin_stem: 'pumpkin_seeds',
	attached_pumpkin_stem: 'pumpkin_seeds',
	torchflower_crop: 'torchflower_seeds',
	cocoa: 'cocoa_beans',
	sweet_berry_bush: 'sweet_berries',
	cave_vines: 'glow_berries',
	cave_vines_plant: 'glow_berries',
	kelp_plant: 'kelp',
	bamboo_sapling: 'bamboo',
	big_dripleaf_stem: 'big_dripleaf',
	weeping_vines_plant: 'weeping_vines',
	twisting_vines_plant: 'twisting_vines',
	water_cauldron: 'cauldron',
	lava_cauldron: 'cauldron',
	powder_snow_cauldron: 'cauldron',
	wall_torch: 'torch',
	soul_wall_torch: 'soul_torch',
	redstone_wall_torch: 'redstone_torch',
	potted_azalea_bush: 'azalea',
	potted_flowering_azalea_bush: 'flowering_azalea',
}

// Blocks that are two blocks tall: only the lower half counts as an item.
const TALL = new Set([
	'tall_grass',
	'large_fern',
	'sunflower',
	'lilac',
	'rose_bush',
	'peony',
	'tall_seagrass',
	'pitcher_plant',
])

/** The items one block needs, as [item, count per block]. */
export function itemsForBlock(state: BlockState): { id: string; per: number }[] {
	if (isAirLike(state.name)) return []
	const [namespace, path] = state.name.includes(':') ? state.name.split(':') : ['minecraft', state.name]
	const properties = state.properties
	const make = (item: string, per = 1) => ({ id: `${namespace}:${item}`, per })

	if (NO_ITEM.has(path)) return []

	// The second half of beds and doors, and the top of tall plants, is not an item of its own.
	if (path.endsWith('_bed')) return properties.part === 'head' ? [make(path)] : []
	if (path.endsWith('_door')) return properties.half === 'lower' ? [make(path)] : []
	if (TALL.has(path)) return properties.half === 'lower' ? [make(path)] : []

	if (path.endsWith('_slab')) return [make(path, properties.type === 'double' ? 2 : 1)]
	if (path === 'snow') return [make('snow', parseInt(properties.layers ?? '1', 10) || 1)]
	if (path === 'sea_pickle') return [make(path, parseInt(properties.pickles ?? '1', 10) || 1)]
	if (path === 'turtle_egg') return [make(path, parseInt(properties.eggs ?? '1', 10) || 1)]
	if (path === 'candle' || (path.endsWith('_candle') && !path.endsWith('_candle_cake'))) {
		return [make(path, parseInt(properties.candles ?? '1', 10) || 1)]
	}
	if (path.endsWith('_candle_cake')) return [make('cake'), make(path.replace('_candle_cake', '_candle'))]

	if (path.startsWith('potted_')) {
		const plant = ITEM_FOR_BLOCK[path] ?? path.slice('potted_'.length)
		return [make('flower_pot'), make(plant)]
	}

	// Wall variants of signs, banners and heads
	const wall = path.match(/^(.*)_wall_(sign|hanging_sign|banner|head|skull)$/)
	if (wall) return [make(`${wall[1]}_${wall[2]}`)]

	return [make(ITEM_FOR_BLOCK[path] ?? path)]
}

export function displayName(id: string): string {
	const path = id.includes(':') ? id.split(':')[1] : id
	return path
		.split('_')
		.map((word) => (word ? word[0].toUpperCase() + word.slice(1) : word))
		.join(' ')
}

const pause = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

/**
 * Counts everything a schematic needs. Large schematics are worked through in
 * slices, so the page stays responsive while it runs.
 */
export async function countMaterials(
	schematic: ParsedSchematic,
	isCancelled: () => boolean = () => false,
): Promise<MaterialEntry[]> {
	const totals = new Map<string, number>()
	const SLICE = 1_500_000

	for (const region of schematic.regions) {
		const cells = Math.abs(region.size.x) * Math.abs(region.size.y) * Math.abs(region.size.z)
		const perPalette = new Float64Array(region.palette.length)

		for (let start = 0; start < cells; start += SLICE) {
			const end = Math.min(cells, start + SLICE)
			for (let i = start; i < end; i++) {
				const index = blockIndexAt(region, i)
				if (index < perPalette.length) perPalette[index]++
			}
			await pause()
			if (isCancelled()) return []
		}

		region.palette.forEach((state, index) => {
			const blocks = perPalette[index]
			if (!blocks) return
			for (const item of itemsForBlock(state)) {
				totals.set(item.id, (totals.get(item.id) ?? 0) + blocks * item.per)
			}
		})
	}

	return [...totals.entries()]
		.map(([id, count]) => ({ id, name: displayName(id), count }))
		.sort((a, b) => b.count - a.count)
}

/** "3 stacks + 12" for a count (stacks of 64). */
export function stackText(count: number): string {
	const stacks = Math.floor(count / 64)
	const rest = count % 64
	if (stacks === 0) return ''
	return rest === 0 ? `${stacks} × 64` : `${stacks} × 64 + ${rest}`
}
