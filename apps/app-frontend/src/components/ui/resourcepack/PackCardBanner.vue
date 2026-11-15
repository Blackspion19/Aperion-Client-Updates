<script setup lang="ts">
import { LayersIcon } from '@aperion/assets'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { read_resourcepack_asset, type ResourcePackMetadata } from '@/helpers/resourcepack'

const props = defineProps<{
	pack: ResourcePackMetadata
}>()

const iconUrl = ref<string | null>(null)
// Two main colors of the icon, as [hue, saturation 0-1]
const colors = ref<{ h: number; s: number }[]>([])

function fallbackColors(id: string): { h: number; s: number }[] {
	let hash = 0
	for (let i = 0; i < id.length; i++) hash = (hash * 31 + id.charCodeAt(i)) | 0
	const hue = Math.abs(hash) % 360
	return [
		{ h: hue, s: 0.45 },
		{ h: (hue + 40) % 360, s: 0.45 },
	]
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
	const rn = r / 255
	const gn = g / 255
	const bn = b / 255
	const max = Math.max(rn, gn, bn)
	const min = Math.min(rn, gn, bn)
	const l = (max + min) / 2
	if (max === min) return [0, 0, l]
	const d = max - min
	const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
	let h = 0
	if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0)
	else if (max === gn) h = (bn - rn) / d + 2
	else h = (rn - gn) / d + 4
	return [h * 60, s, l]
}

/** The two most striking colors of an image (ignores transparent, white and black pixels). */
function extractColors(bitmap: ImageBitmap): { h: number; s: number }[] {
	const size = 32
	const canvas = document.createElement('canvas')
	canvas.width = size
	canvas.height = size
	const context = canvas.getContext('2d', { willReadFrequently: true })
	if (!context) return []
	context.drawImage(bitmap, 0, 0, size, size)
	const data = context.getImageData(0, 0, size, size).data

	// Group pixels into 36 hue buckets; saturated, bright colors count more.
	const buckets = Array.from({ length: 36 }, () => ({ weight: 0, hue: 0, sat: 0 }))
	for (let i = 0; i < data.length; i += 4) {
		if (data[i + 3] < 128) continue
		const [h, s, l] = rgbToHsl(data[i], data[i + 1], data[i + 2])
		if (l < 0.12 || l > 0.92 || s < 0.18) continue
		const weight = s * (1 - Math.abs(l - 0.5))
		const bucket = buckets[Math.floor(h / 10) % 36]
		bucket.weight += weight
		bucket.hue += h * weight
		bucket.sat += s * weight
	}
	const ranked = buckets
		.filter((bucket) => bucket.weight > 0)
		.map((bucket) => ({ h: bucket.hue / bucket.weight, s: bucket.sat / bucket.weight, w: bucket.weight }))
		.sort((a, b) => b.w - a.w)
	if (ranked.length === 0) return []

	const first = ranked[0]
	// A second color that is clearly different from the first one
	const second = ranked.find((color) => {
		const distance = Math.min(Math.abs(color.h - first.h), 360 - Math.abs(color.h - first.h))
		return distance > 35
	})
	return [
		{ h: first.h, s: first.s },
		second ? { h: second.h, s: second.s } : { h: (first.h + 30) % 360, s: first.s },
	]
}

let loadTicket = 0

async function loadIcon() {
	const ticket = ++loadTicket
	if (iconUrl.value) {
		URL.revokeObjectURL(iconUrl.value)
		iconUrl.value = null
	}
	colors.value = []
	if (!props.pack.icon_path) return
	try {
		const bytes = await read_resourcepack_asset(props.pack.id, 'pack.png')
		const blob = new Blob([bytes as BlobPart], { type: 'image/png' })
		const bitmap = await createImageBitmap(blob)
		const extracted = extractColors(bitmap)
		bitmap.close()
		if (ticket !== loadTicket) return
		iconUrl.value = URL.createObjectURL(blob)
		colors.value = extracted
	} catch {
		// no readable icon: the banner uses its own color
	}
}

watch(() => [props.pack.id, props.pack.icon_path], loadIcon, { immediate: true })
onBeforeUnmount(() => {
	loadTicket++
	if (iconUrl.value) URL.revokeObjectURL(iconUrl.value)
})

const palette = computed(() => (colors.value.length === 2 ? colors.value : fallbackColors(props.pack.id)))

const style = computed(() => {
	const [a, b] = palette.value
	const sa = Math.round(Math.min(a.s, 0.75) * 100)
	const sb = Math.round(Math.min(b.s, 0.75) * 100)
	return {
		background: [
			`radial-gradient(circle at 50% 55%, hsl(${Math.round(a.h)} ${sa}% 45% / 0.55) 0%, transparent 62%)`,
			`linear-gradient(135deg, hsl(${Math.round(a.h)} ${sa}% 30%) 0%, hsl(${Math.round(b.h)} ${sb}% 17%) 100%)`,
		].join(', '),
	}
})
</script>
<template>
	<div class="relative flex aspect-[16/9] items-center justify-center overflow-hidden" :style="style">
		<img
			v-if="iconUrl"
			:src="iconUrl"
			class="size-20 rounded-2xl object-cover shadow-lg ring-2 ring-white/20 [image-rendering:pixelated]"
			alt=""
		/>
		<LayersIcon v-else class="size-14 text-white/70" />
		<slot />
	</div>
</template>
