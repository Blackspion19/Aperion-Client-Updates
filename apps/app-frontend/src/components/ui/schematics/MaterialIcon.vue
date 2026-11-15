<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { vanillaPreview } from '@/helpers/resourcepack-previews'

const props = defineProps<{
	id: string
	gameVersion: string
}>()

const root = ref<HTMLElement>()
const src = ref<string | null>(null)
const visible = ref(false)
let observer: IntersectionObserver | undefined

onMounted(() => {
	observer = new IntersectionObserver(
		(entries) => {
			if (entries.some((entry) => entry.isIntersecting)) {
				visible.value = true
				observer?.disconnect()
			}
		},
		{ rootMargin: '200px' },
	)
	if (root.value) observer.observe(root.value)
})
onBeforeUnmount(() => observer?.disconnect())

function candidates(): string[] {
	const name = props.id.includes(':') ? props.id.split(':')[1] : props.id
	return [`block/${name}`, `item/${name}`, `block/${name}_front`, `block/${name}_top`, `block/${name}_side`]
}

// Only icons on screen are loaded. The first texture that exists is used.
watch(
	[visible, () => props.id, () => props.gameVersion],
	async () => {
		if (!visible.value || !props.gameVersion) return
		src.value = null
		for (const candidate of candidates()) {
			const url = await vanillaPreview(props.gameVersion, `assets/minecraft/textures/${candidate}.png`)
			if (url) {
				src.value = url
				return
			}
		}
	},
	{ immediate: true },
)
</script>
<template>
	<div ref="root" class="flex size-8 shrink-0 items-center justify-center overflow-hidden rounded bg-surface-4">
		<img
			v-if="src"
			:src="src"
			class="size-full object-contain [image-rendering:pixelated]"
			alt=""
			decoding="async"
		/>
	</div>
</template>
