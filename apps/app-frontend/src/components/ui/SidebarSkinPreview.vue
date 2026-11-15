<script setup lang="ts">
import { SkinPreviewRenderer } from '@aperion/ui'
import { onMounted, ref } from 'vue'

import {
	type Cape,
	get_available_capes,
	get_available_skins,
	get_normalized_skin_texture,
	type Skin,
} from '@/helpers/skins'

// Shows the skin that is currently equipped (with its cape). The model can
// be dragged to rotate; there is nothing else to interact with.
const texture = ref('')
const variant = ref<Skin['variant']>('CLASSIC')
const capeTexture = ref<string | undefined>(undefined)
let loadId = 0

async function refresh() {
	const ticket = ++loadId
	try {
		const [skins, capes] = await Promise.all([
			get_available_skins(),
			get_available_capes().catch(() => [] as Cape[]),
		])
		const equipped = (skins ?? []).find((skin) => skin.is_equipped)
		if (!equipped) {
			if (ticket === loadId) texture.value = ''
			return
		}

		let normalized = ''
		try {
			normalized = await get_normalized_skin_texture(equipped)
		} catch {
			normalized = equipped.texture.startsWith('data:image/') ? equipped.texture : ''
		}

		const cape =
			(equipped.cape_id && capes.find((c) => c.id === equipped.cape_id)) ||
			capes.find((c) => c.is_equipped)

		// A newer refresh (for example after an account switch) wins.
		if (ticket !== loadId) return
		texture.value = normalized
		variant.value = equipped.variant
		capeTexture.value = cape?.texture
	} catch {
		// Offline or not signed in: keep showing whatever was there before.
	}
}

onMounted(refresh)
defineExpose({ refresh })
</script>
<template>
	<div class="flex h-full min-h-0 w-full items-center justify-center">
		<SkinPreviewRenderer
			v-if="texture"
			:texture-src="texture"
			:cape-src="capeTexture"
			:variant="variant"
			:initial-rotation="Math.PI / 8"
			class="h-full w-full"
		/>
	</div>
</template>
