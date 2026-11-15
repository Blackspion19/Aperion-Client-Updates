<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { packPreview, vanillaPreview } from '@/helpers/resourcepack-previews'

const props = defineProps<{
	gameVersion: string
	packId: string
	path: string
	overridden: boolean
	revision: number
}>()

const root = ref<HTMLElement>()
const src = ref<string | null>(null)
const visible = ref(false)
let observer: IntersectionObserver | undefined
let latest = 0

onMounted(() => {
	observer = new IntersectionObserver(
		(entries) => {
			if (entries.some((entry) => entry.isIntersecting)) {
				visible.value = true
				observer?.disconnect()
			}
		},
		{ rootMargin: '300px' },
	)
	if (root.value) observer.observe(root.value)
})
onBeforeUnmount(() => observer?.disconnect())

// Only tiles that are on (or near) the screen load their image.
watch(
	[visible, () => props.overridden, () => props.revision, () => props.path, () => props.gameVersion],
	async () => {
		if (!visible.value) return
		const ticket = ++latest
		const url = props.overridden
			? await packPreview(props.packId, props.path, props.revision)
			: await vanillaPreview(props.gameVersion, props.path)
		if (ticket === latest) src.value = url
	},
	{ immediate: true },
)
</script>
<template>
	<div ref="root" class="size-10 shrink-0 overflow-hidden rounded bg-bg-input">
		<img
			v-if="src"
			:src="src"
			class="size-full object-contain [image-rendering:pixelated]"
			alt=""
			decoding="async"
		/>
	</div>
</template>
