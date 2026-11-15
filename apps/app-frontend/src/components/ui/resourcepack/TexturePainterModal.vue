<script setup lang="ts">
import { CopyIcon, SaveIcon, XIcon } from '@aperion/assets'
import { Button, defineMessages, injectNotificationManager, NewModal, useVIntl } from '@aperion/ui'
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'

import { copiedTexture } from '@/helpers/texture-clipboard'
import {
	read_resourcepack_asset,
	read_vanilla_asset,
	type ResourcePackMetadata,
	set_resourcepack_texture,
} from '@/helpers/resourcepack'

const props = defineProps<{
	pack: ResourcePackMetadata
}>()

const emit = defineEmits<{
	saved: [pack: ResourcePackMetadata, path: string]
}>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'app.resourcepacks.painter.title', defaultMessage: 'Paint texture' },
	pencil: { id: 'app.resourcepacks.painter.pencil', defaultMessage: 'Pencil' },
	eraser: { id: 'app.resourcepacks.painter.eraser', defaultMessage: 'Eraser' },
	fill: { id: 'app.resourcepacks.painter.fill', defaultMessage: 'Fill' },
	picker: { id: 'app.resourcepacks.painter.picker', defaultMessage: 'Pick color' },
	size: { id: 'app.resourcepacks.painter.size', defaultMessage: 'Size' },
	opacity: { id: 'app.resourcepacks.painter.opacity', defaultMessage: 'Opacity' },
	zoom: { id: 'app.resourcepacks.painter.zoom', defaultMessage: 'Zoom' },
	grid: { id: 'app.resourcepacks.painter.grid', defaultMessage: 'Grid' },
	mirrorX: { id: 'app.resourcepacks.painter.mirror-x', defaultMessage: 'Mirror left/right' },
	mirrorY: { id: 'app.resourcepacks.painter.mirror-y', defaultMessage: 'Mirror top/bottom' },
	undo: { id: 'app.resourcepacks.painter.undo', defaultMessage: 'Undo' },
	redo: { id: 'app.resourcepacks.painter.redo', defaultMessage: 'Redo' },
	copy: { id: 'app.resourcepacks.painter.copy', defaultMessage: 'Copy texture' },
	paste: { id: 'app.resourcepacks.painter.paste', defaultMessage: 'Paste texture' },
	copied: { id: 'app.resourcepacks.painter.copied', defaultMessage: 'Texture copied' },
	pasted: { id: 'app.resourcepacks.painter.pasted', defaultMessage: 'Texture pasted' },
	nothingCopied: {
		id: 'app.resourcepacks.painter.nothing-copied',
		defaultMessage: 'Nothing copied yet. Copy a texture first.',
	},
	pasteHint: {
		id: 'app.resourcepacks.painter.paste-hint',
		defaultMessage: 'Paste replaces the whole texture. A different size is scaled to fit.',
	},
	flipH: { id: 'app.resourcepacks.painter.flip-h', defaultMessage: 'Flip horizontally' },
	flipV: { id: 'app.resourcepacks.painter.flip-v', defaultMessage: 'Flip vertically' },
	clear: { id: 'app.resourcepacks.painter.clear', defaultMessage: 'Clear' },
	original: { id: 'app.resourcepacks.painter.original', defaultMessage: 'Reset to original' },
	save: { id: 'app.resourcepacks.painter.save', defaultMessage: 'Save texture' },
	cancel: { id: 'app.resourcepacks.painter.cancel', defaultMessage: 'Cancel' },
	loadFailed: {
		id: 'app.resourcepacks.painter.load-failed',
		defaultMessage: 'This texture could not be opened.',
	},
	recent: { id: 'app.resourcepacks.painter.recent', defaultMessage: 'Recent colors' },
})

type Tool = 'pencil' | 'eraser' | 'fill' | 'picker'

const modal = ref<InstanceType<typeof NewModal>>()
const canvas = ref<HTMLCanvasElement>()

const path = ref('')
const width = ref(16)
const height = ref(16)
const loadError = ref(false)
const saving = ref(false)
const dirty = ref(false)

const tool = ref<Tool>('pencil')
const color = ref('#ff0000')
const alpha = ref(255)
const brushSize = ref(1)
const mirrorX = ref(false)
const mirrorY = ref(false)
const showGrid = ref(true)
const zoom = ref(1)
const recentColors = ref<string[]>([])

// The pixels live in a plain ImageData (not reactive); the canvas shows them.
let pixels: ImageData | null = null
let originalBytes: Uint8Array | null = null
let undoStack: ImageData[] = []
let redoStack: ImageData[] = []
const historyVersion = ref(0)
const canUndo = computed(() => historyVersion.value >= 0 && undoStack.length > 0)
const canRedo = computed(() => historyVersion.value >= 0 && redoStack.length > 0)

// Small textures are shown big, large ones at (almost) native size.
const baseScale = computed(() => Math.max(1, Math.floor(480 / Math.max(width.value, height.value))))
const scale = computed(() => baseScale.value * zoom.value)
const gridStyle = computed(() => {
	if (!showGrid.value || scale.value < 6) return {}
	const size = `${scale.value}px ${scale.value}px`
	return {
		backgroundImage:
			'linear-gradient(to right, rgba(128,128,128,.35) 1px, transparent 1px), linear-gradient(to bottom, rgba(128,128,128,.35) 1px, transparent 1px)',
		backgroundSize: size,
	}
})

function hexToRgb(hex: string): [number, number, number] {
	const value = hex.replace('#', '')
	return [
		parseInt(value.slice(0, 2), 16) || 0,
		parseInt(value.slice(2, 4), 16) || 0,
		parseInt(value.slice(4, 6), 16) || 0,
	]
}

function rgbToHex(r: number, g: number, b: number): string {
	return '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')
}

function snapshot(): ImageData | null {
	if (!pixels) return null
	return new ImageData(new Uint8ClampedArray(pixels.data), pixels.width, pixels.height)
}

function pushUndo() {
	const copy = snapshot()
	if (!copy) return
	undoStack.push(copy)
	if (undoStack.length > 50) undoStack.shift()
	redoStack = []
	historyVersion.value++
	dirty.value = true
}

function redraw() {
	const context = canvas.value?.getContext('2d')
	if (context && pixels) context.putImageData(pixels, 0, 0)
}

function undo() {
	const previous = undoStack.pop()
	const current = snapshot()
	if (!previous || !current) return
	redoStack.push(current)
	pixels = previous
	historyVersion.value++
	redraw()
}

function redo() {
	const next = redoStack.pop()
	const current = snapshot()
	if (!next || !current) return
	undoStack.push(current)
	pixels = next
	historyVersion.value++
	redraw()
}

async function loadBytes(bytes: Uint8Array) {
	const bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: 'image/png' }))
	width.value = bitmap.width
	height.value = bitmap.height
	await nextTick()
	const context = canvas.value?.getContext('2d', { willReadFrequently: true })
	if (!context) throw new Error('No canvas')
	context.clearRect(0, 0, bitmap.width, bitmap.height)
	context.drawImage(bitmap, 0, 0)
	pixels = context.getImageData(0, 0, bitmap.width, bitmap.height)
	bitmap.close()
}

async function show(assetPath: string, overridden: boolean) {
	path.value = assetPath
	loadError.value = false
	dirty.value = false
	undoStack = []
	redoStack = []
	historyVersion.value++
	zoom.value = 1
	pixels = null
	modal.value?.show()
	window.addEventListener('keydown', onKeydown)
	try {
		// An already customized texture opens as it currently is in the pack.
		const bytes = overridden
			? await read_resourcepack_asset(props.pack.id, assetPath)
			: await read_vanilla_asset(props.pack.game_version, assetPath)
		originalBytes = overridden
			? await read_vanilla_asset(props.pack.game_version, assetPath).catch(() => null)
			: bytes
		await loadBytes(bytes)
	} catch {
		loadError.value = true
	}
}

function cleanup() {
	window.removeEventListener('keydown', onKeydown)
}

function close() {
	modal.value?.hide()
}

onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))

function onKeydown(event: KeyboardEvent) {
	if (!(event.ctrlKey || event.metaKey)) return
	const key = event.key.toLowerCase()
	if (key === 'z' && !event.shiftKey) {
		event.preventDefault()
		undo()
	} else if (key === 'y' || (key === 'z' && event.shiftKey)) {
		event.preventDefault()
		redo()
	}
}

// ---- drawing ----

function setPixel(x: number, y: number, r: number, g: number, b: number, a: number) {
	if (!pixels || x < 0 || y < 0 || x >= pixels.width || y >= pixels.height) return
	const index = (y * pixels.width + x) * 4
	pixels.data[index] = r
	pixels.data[index + 1] = g
	pixels.data[index + 2] = b
	pixels.data[index + 3] = a
}

function stamp(x: number, y: number) {
	if (!pixels) return
	const erase = tool.value === 'eraser'
	const [r, g, b] = hexToRgb(color.value)
	const a = erase ? 0 : alpha.value
	const offset = Math.floor((brushSize.value - 1) / 2)
	for (let dy = 0; dy < brushSize.value; dy++) {
		for (let dx = 0; dx < brushSize.value; dx++) {
			const px = x - offset + dx
			const py = y - offset + dy
			const xs = mirrorX.value ? [px, pixels.width - 1 - px] : [px]
			const ys = mirrorY.value ? [py, pixels.height - 1 - py] : [py]
			for (const mx of xs) for (const my of ys) setPixel(mx, my, r, g, b, a)
		}
	}
}

function line(x0: number, y0: number, x1: number, y1: number) {
	// Bresenham, so fast mouse movement leaves no gaps.
	const dx = Math.abs(x1 - x0)
	const dy = -Math.abs(y1 - y0)
	const sx = x0 < x1 ? 1 : -1
	const sy = y0 < y1 ? 1 : -1
	let error = dx + dy
	let x = x0
	let y = y0
	for (;;) {
		stamp(x, y)
		if (x === x1 && y === y1) break
		const doubled = 2 * error
		if (doubled >= dy) {
			error += dy
			x += sx
		}
		if (doubled <= dx) {
			error += dx
			y += sy
		}
	}
}

function floodFill(startX: number, startY: number) {
	if (!pixels) return
	const { width: w, height: h, data } = pixels
	const start = (startY * w + startX) * 4
	const target = [data[start], data[start + 1], data[start + 2], data[start + 3]]
	const [r, g, b] = hexToRgb(color.value)
	const a = alpha.value
	if (target[0] === r && target[1] === g && target[2] === b && target[3] === a) return

	const matches = (index: number) =>
		data[index] === target[0] &&
		data[index + 1] === target[1] &&
		data[index + 2] === target[2] &&
		data[index + 3] === target[3]

	const stack = [startX, startY]
	while (stack.length) {
		const y = stack.pop() as number
		const x = stack.pop() as number
		if (x < 0 || y < 0 || x >= w || y >= h) continue
		const index = (y * w + x) * 4
		if (!matches(index)) continue
		setPixel(x, y, r, g, b, a)
		stack.push(x + 1, y, x - 1, y, x, y + 1, x, y - 1)
	}
}

function pickColor(x: number, y: number) {
	if (!pixels || x < 0 || y < 0 || x >= pixels.width || y >= pixels.height) return
	const index = (y * pixels.width + x) * 4
	const [r, g, b, a] = [
		pixels.data[index],
		pixels.data[index + 1],
		pixels.data[index + 2],
		pixels.data[index + 3],
	]
	if (a === 0) return
	color.value = rgbToHex(r, g, b)
	alpha.value = a
	tool.value = 'pencil'
}

function rememberColor() {
	const value = color.value
	recentColors.value = [value, ...recentColors.value.filter((c) => c !== value)].slice(0, 12)
}

let drawing = false
let lastX = 0
let lastY = 0

function pointerPixel(event: PointerEvent): [number, number] {
	const rect = canvas.value!.getBoundingClientRect()
	return [
		Math.floor(((event.clientX - rect.left) * width.value) / rect.width),
		Math.floor(((event.clientY - rect.top) * height.value) / rect.height),
	]
}

function onPointerDown(event: PointerEvent) {
	if (!pixels || event.button !== 0) return
	const [x, y] = pointerPixel(event)
	if (tool.value === 'picker') {
		pickColor(x, y)
		return
	}
	pushUndo()
	canvas.value?.setPointerCapture(event.pointerId)
	if (tool.value === 'fill') {
		if (x >= 0 && y >= 0 && x < width.value && y < height.value) {
			floodFill(x, y)
			rememberColor()
			redraw()
		}
		return
	}
	drawing = true
	lastX = x
	lastY = y
	if (tool.value === 'pencil') rememberColor()
	stamp(x, y)
	redraw()
}

function onPointerMove(event: PointerEvent) {
	if (!drawing) return
	const [x, y] = pointerPixel(event)
	if (x === lastX && y === lastY) return
	line(lastX, lastY, x, y)
	lastX = x
	lastY = y
	redraw()
}

function onPointerUp() {
	drawing = false
}

// ---- whole-image actions ----

function flip(horizontal: boolean) {
	if (!pixels) return
	pushUndo()
	const { width: w, height: h, data } = pixels
	const copy = new Uint8ClampedArray(data)
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			const sx = horizontal ? w - 1 - x : x
			const sy = horizontal ? y : h - 1 - y
			const from = (sy * w + sx) * 4
			const to = (y * w + x) * 4
			for (let c = 0; c < 4; c++) data[to + c] = copy[from + c]
		}
	}
	redraw()
}

function clearAll() {
	if (!pixels) return
	pushUndo()
	pixels.data.fill(0)
	redraw()
}

const notice = ref('')
let noticeTimer: ReturnType<typeof setTimeout> | undefined

function flash(text: string) {
	notice.value = text
	clearTimeout(noticeTimer)
	noticeTimer = setTimeout(() => (notice.value = ''), 2200)
}

onBeforeUnmount(() => clearTimeout(noticeTimer))

// Copies what is currently on the canvas, with all unsaved changes.
async function copyTexture() {
	const element = canvas.value
	if (!element) return
	try {
		const blob = await new Promise<Blob | null>((resolve) => element.toBlob(resolve, 'image/png'))
		if (!blob) return
		copiedTexture.value = {
			source: path.value,
			png: new Uint8Array(await blob.arrayBuffer()),
			width: element.width,
			height: element.height,
		}
		// Also put the image on the system clipboard, so it can be used in other programs.
		try {
			await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
		} catch {
			// not available here: the in-app copy still works
		}
		flash(formatMessage(messages.copied))
	} catch (e) {
		handleError(e)
	}
}

async function pasteTexture() {
	const element = canvas.value
	const context = element?.getContext('2d', { willReadFrequently: true })
	if (!element || !context) return

	let blob: Blob | null = null
	const clip = copiedTexture.value
	if (clip) {
		blob = new Blob([clip.png as BlobPart], { type: 'image/png' })
	} else {
		// No in-app copy: try an image from the system clipboard.
		try {
			for (const item of await navigator.clipboard.read()) {
				const type = item.types.find((entry) => entry.startsWith('image/'))
				if (type) {
					blob = await item.getType(type)
					break
				}
			}
		} catch {
			// no permission or no image
		}
	}
	if (!blob) {
		flash(formatMessage(messages.nothingCopied))
		return
	}

	try {
		const bitmap = await createImageBitmap(blob)
		pushUndo()
		context.save()
		context.imageSmoothingEnabled = false
		context.clearRect(0, 0, element.width, element.height)
		// Same size: 1:1. Other sizes are stretched with hard pixel edges.
		context.drawImage(bitmap, 0, 0, element.width, element.height)
		context.restore()
		bitmap.close()
		pixels = context.getImageData(0, 0, element.width, element.height)
		flash(formatMessage(messages.pasted))
	} catch (e) {
		handleError(e)
	}
}

async function resetToOriginal() {
	if (!originalBytes) return
	try {
		pushUndo()
		await loadBytes(originalBytes)
	} catch {
		loadError.value = true
	}
}

async function save() {
	if (!canvas.value || !pixels || saving.value) return
	saving.value = true
	try {
		const blob = await new Promise<Blob | null>((resolve) =>
			canvas.value!.toBlob(resolve, 'image/png'),
		)
		if (!blob) throw new Error('PNG export failed')
		const bytes = new Uint8Array(await blob.arrayBuffer())
		const updated = await set_resourcepack_texture(props.pack.id, path.value, bytes)
		emit('saved', updated, path.value)
		close()
	} catch (e) {
		handleError(e)
	} finally {
		saving.value = false
	}
}

defineExpose({ show })

const tools = computed<{ id: Tool; label: string }[]>(() => [
	{ id: 'pencil', label: formatMessage(messages.pencil) },
	{ id: 'eraser', label: formatMessage(messages.eraser) },
	{ id: 'fill', label: formatMessage(messages.fill) },
	{ id: 'picker', label: formatMessage(messages.picker) },
])

const shortName = computed(() => path.value.replace(/^assets\/minecraft\//, ''))
</script>
<template>
	<NewModal
		ref="modal"
		:header="`${formatMessage(messages.title)}: ${shortName}`"
		width="760px"
		max-width="760px"
		:on-hide="cleanup"
		:close-on-click-outside="false"
	>
		<p v-if="loadError" class="m-0 text-sm text-red">{{ formatMessage(messages.loadFailed) }}</p>

		<div v-show="!loadError" class="flex flex-col gap-3">
			<div class="flex flex-wrap items-center gap-2">
				<Button
					v-for="entry in tools"
					:key="entry.id"
					:type="tool === entry.id ? 'colored' : 'outlined'"
					:color="tool === entry.id ? 'brand' : undefined"
					@click="tool = entry.id"
				>
					{{ entry.label }}
				</Button>
				<span class="mx-1 h-6 w-px bg-surface-5"></span>
				<Button type="outlined" :disabled="!canUndo" @click="undo">{{ formatMessage(messages.undo) }}</Button>
				<Button type="outlined" :disabled="!canRedo" @click="redo">{{ formatMessage(messages.redo) }}</Button>
			</div>

			<div class="flex flex-wrap items-center gap-4 text-sm text-secondary">
				<label class="flex items-center gap-2">
					<input v-model="color" type="color" class="h-8 w-10 cursor-pointer rounded border-0 bg-transparent p-0" />
					<span class="font-mono">{{ color }}</span>
				</label>
				<label class="flex items-center gap-2">
					{{ formatMessage(messages.opacity) }}
					<input v-model.number="alpha" type="range" min="1" max="255" class="w-24" />
				</label>
				<label class="flex items-center gap-2">
					{{ formatMessage(messages.size) }} {{ brushSize }}
					<input v-model.number="brushSize" type="range" min="1" max="8" class="w-24" />
				</label>
				<label class="flex items-center gap-2">
					{{ formatMessage(messages.zoom) }}
					<input v-model.number="zoom" type="range" min="1" max="4" step="0.5" class="w-24" />
				</label>
				<label class="flex items-center gap-1"><input v-model="showGrid" type="checkbox" />{{ formatMessage(messages.grid) }}</label>
				<label class="flex items-center gap-1"><input v-model="mirrorX" type="checkbox" />{{ formatMessage(messages.mirrorX) }}</label>
				<label class="flex items-center gap-1"><input v-model="mirrorY" type="checkbox" />{{ formatMessage(messages.mirrorY) }}</label>
			</div>

			<div v-if="recentColors.length" class="flex flex-wrap items-center gap-1">
				<span class="mr-1 text-xs text-secondary">{{ formatMessage(messages.recent) }}</span>
				<button
					v-for="recent in recentColors"
					:key="recent"
					class="size-5 cursor-pointer rounded border border-solid border-surface-5"
					:style="{ backgroundColor: recent }"
					@click="color = recent"
				/>
			</div>

			<div class="max-h-[480px] overflow-auto rounded-2xl border border-solid border-surface-4 bg-bg-input p-3 shadow-inner">
				<div
					class="relative mx-auto w-fit"
					:style="{
						width: `${width * scale}px`,
						height: `${height * scale}px`,
						backgroundImage:
							'conic-gradient(#8884 25%, transparent 0 50%, #8884 0 75%, transparent 0)',
						backgroundSize: '16px 16px',
					}"
				>
					<canvas
						ref="canvas"
						:width="width"
						:height="height"
						class="block touch-none [image-rendering:pixelated]"
						:style="{ width: `${width * scale}px`, height: `${height * scale}px`, cursor: 'crosshair' }"
						@pointerdown="onPointerDown"
						@pointermove="onPointerMove"
						@pointerup="onPointerUp"
						@pointercancel="onPointerUp"
					/>
					<div class="pointer-events-none absolute inset-0" :style="gridStyle"></div>
				</div>
			</div>

			<div class="flex flex-wrap gap-2">
				<Button type="outlined" @click="copyTexture">
					<CopyIcon />
					{{ formatMessage(messages.copy) }}
				</Button>
				<Button type="outlined" :title="formatMessage(messages.pasteHint)" @click="pasteTexture">
					{{ formatMessage(messages.paste) }}
				</Button>
				<span v-if="notice" class="self-center text-sm text-secondary">{{ notice }}</span>
				<Button type="outlined" @click="flip(true)">{{ formatMessage(messages.flipH) }}</Button>
				<Button type="outlined" @click="flip(false)">{{ formatMessage(messages.flipV) }}</Button>
				<Button type="outlined" @click="clearAll">{{ formatMessage(messages.clear) }}</Button>
				<Button v-if="originalBytes" type="outlined" @click="resetToOriginal">
					{{ formatMessage(messages.original) }}
				</Button>
			</div>
		</div>

		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="close">
					<XIcon />
					{{ formatMessage(messages.cancel) }}
				</Button>
				<Button type="colored" color="brand" :disabled="loadError || saving" @click="save">
					<SaveIcon />
					{{ formatMessage(messages.save) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
