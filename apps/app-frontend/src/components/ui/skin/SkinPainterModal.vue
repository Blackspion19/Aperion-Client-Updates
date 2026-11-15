<script setup lang="ts">
import { EyeIcon, EyeOffIcon, XIcon } from '@aperion/assets'
import { Button, defineMessages, NewModal, SkinPreviewRenderer, useVIntl } from '@aperion/ui'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

const { formatMessage } = useVIntl()

const emit = defineEmits<{
	use: [png: Uint8Array]
}>()

const messages = defineMessages({
	title: { id: 'app.skins.painter.title', defaultMessage: 'Draw a skin' },
	pencil: { id: 'app.skins.painter.pencil', defaultMessage: 'Pencil' },
	eraser: { id: 'app.skins.painter.eraser', defaultMessage: 'Eraser' },
	fill: { id: 'app.skins.painter.fill', defaultMessage: 'Fill' },
	picker: { id: 'app.skins.painter.picker', defaultMessage: 'Pick color' },
	undo: { id: 'app.skins.painter.undo', defaultMessage: 'Undo' },
	redo: { id: 'app.skins.painter.redo', defaultMessage: 'Redo' },
	opacity: { id: 'app.skins.painter.opacity', defaultMessage: 'Opacity' },
	size: { id: 'app.skins.painter.size', defaultMessage: 'Size {value}' },
	zoom: { id: 'app.skins.painter.zoom', defaultMessage: 'Zoom' },
	grid: { id: 'app.skins.painter.grid', defaultMessage: 'Grid' },
	layer: { id: 'app.skins.painter.layer', defaultMessage: 'Layer' },
	layerBase: { id: 'app.skins.painter.layer.base', defaultMessage: 'Base' },
	layerOverlay: { id: 'app.skins.painter.layer.overlay', defaultMessage: 'Overlay' },
	layerBoth: { id: 'app.skins.painter.layer.both', defaultMessage: 'Both' },
	layerHint: {
		id: 'app.skins.painter.layer.hint',
		defaultMessage:
			'Only the selected layer can be painted. The other one is dimmed. The overlay is the outer layer (jacket, sleeves, hat).',
	},
	parts: { id: 'app.skins.painter.parts', defaultMessage: 'Body parts' },
	partsHint: {
		id: 'app.skins.painter.parts.hint',
		defaultMessage: 'Hide parts to paint the others more easily. Hidden parts are also hidden in the preview.',
	},
	showAll: { id: 'app.skins.painter.show-all', defaultMessage: 'Show all' },
	armStyle: { id: 'app.skins.painter.arm-style', defaultMessage: 'Arm style' },
	classic: { id: 'app.skins.painter.classic', defaultMessage: 'Classic' },
	slim: { id: 'app.skins.painter.slim', defaultMessage: 'Slim' },
	resetLayer: { id: 'app.skins.painter.reset-layer', defaultMessage: 'Reset layer' },
	cancel: { id: 'app.skins.painter.cancel', defaultMessage: 'Cancel' },
	use: { id: 'app.skins.painter.use', defaultMessage: 'Use this skin' },
	hover: { id: 'app.skins.painter.hover', defaultMessage: '{part} · {layer} layer' },
	head: { id: 'app.skins.painter.part.head', defaultMessage: 'Head' },
	body: { id: 'app.skins.painter.part.body', defaultMessage: 'Body' },
	rightArm: { id: 'app.skins.painter.part.right-arm', defaultMessage: 'Right arm' },
	leftArm: { id: 'app.skins.painter.part.left-arm', defaultMessage: 'Left arm' },
	rightLeg: { id: 'app.skins.painter.part.right-leg', defaultMessage: 'Right leg' },
	leftLeg: { id: 'app.skins.painter.part.left-leg', defaultMessage: 'Left leg' },
})

type Tool = 'pencil' | 'eraser' | 'fill' | 'picker'
type LayerMode = 'base' | 'overlay' | 'both'
type PartKey = 'head' | 'body' | 'rightArm' | 'leftArm' | 'rightLeg' | 'leftLeg'

const SIZE = 64
const PARTS: { key: PartKey; base: [number, number]; overlay: [number, number]; size: (arm: number) => [number, number, number] }[] = [
	{ key: 'head', base: [0, 0], overlay: [32, 0], size: () => [8, 8, 8] },
	{ key: 'body', base: [16, 16], overlay: [16, 32], size: () => [8, 12, 4] },
	{ key: 'rightArm', base: [40, 16], overlay: [40, 32], size: (arm) => [arm, 12, 4] },
	{ key: 'leftArm', base: [32, 48], overlay: [48, 48], size: (arm) => [arm, 12, 4] },
	{ key: 'rightLeg', base: [0, 16], overlay: [0, 32], size: () => [4, 12, 4] },
	{ key: 'leftLeg', base: [16, 48], overlay: [0, 48], size: () => [4, 12, 4] },
]

const modal = ref<InstanceType<typeof NewModal>>()
const canvas = ref<HTMLCanvasElement>()

const variant = ref<'CLASSIC' | 'SLIM'>('CLASSIC')
const tool = ref<Tool>('pencil')
const layerMode = ref<LayerMode>('both')
const color = ref('#c0392b')
const alpha = ref(255)
const brushSize = ref(1)
const showGrid = ref(true)
const zoom = ref(1)
const hover = ref('')
const previewUrl = ref('')
// Hidden state per part and layer, e.g. "head:0" (base) or "head:1" (overlay)
const hidden = ref(new Set<string>())
const historyVersion = ref(0)

let pixels = new ImageData(SIZE, SIZE)
let original = new ImageData(SIZE, SIZE)
let owner = new Uint8Array(SIZE * SIZE) // 0 = unused, else 1 + layer * 6 + partIndex
let undoStack: ImageData[] = []
let redoStack: ImageData[] = []

const canUndo = computed(() => historyVersion.value >= 0 && undoStack.length > 0)
const canRedo = computed(() => historyVersion.value >= 0 && redoStack.length > 0)
const scale = computed(() => 8 * zoom.value)

const partLabels = computed(() => ({
	head: formatMessage(messages.head),
	body: formatMessage(messages.body),
	rightArm: formatMessage(messages.rightArm),
	leftArm: formatMessage(messages.leftArm),
	rightLeg: formatMessage(messages.rightLeg),
	leftLeg: formatMessage(messages.leftLeg),
}))

// ---------- skin layout ----------

type Rect = [number, number, number, number]

function faces(u: number, v: number, [w, h, d]: [number, number, number]): Rect[] {
	return [
		[u + d, v, w, d], // top
		[u + d + w, v, w, d], // bottom
		[u, v + d, d, h], // right
		[u + d, v + d, w, h], // front
		[u + d + w, v + d, d, h], // left
		[u + 2 * d + w, v + d, w, h], // back
	]
}

let faceRects: Rect[] = []

function buildOwners() {
	owner = new Uint8Array(SIZE * SIZE)
	faceRects = []
	const arm = variant.value === 'SLIM' ? 3 : 4
	PARTS.forEach((part, index) => {
		for (const layer of [0, 1]) {
			const origin = layer === 0 ? part.base : part.overlay
			for (const rect of faces(origin[0], origin[1], part.size(arm))) {
				faceRects.push(rect)
				const [x, y, w, h] = rect
				for (let py = y; py < y + h; py++) {
					for (let px = x; px < x + w; px++) {
						if (px < SIZE && py < SIZE) owner[py * SIZE + px] = 1 + layer * 6 + index
					}
				}
			}
		}
	})
}

function ownerInfo(id: number): { part: PartKey; layer: 0 | 1 } | null {
	if (id === 0) return null
	const layer = id > 6 ? 1 : 0
	return { part: PARTS[(id - 1) % 6].key, layer }
}

function isHidden(id: number): boolean {
	const info = ownerInfo(id)
	return !!info && hidden.value.has(`${info.part}:${info.layer}`)
}

function isEditable(id: number): boolean {
	const info = ownerInfo(id)
	if (!info || isHidden(id)) return false
	if (layerMode.value === 'both') return true
	return layerMode.value === 'base' ? info.layer === 0 : info.layer === 1
}

// ---------- drawing the editor and the preview ----------

function redraw() {
	const element = canvas.value
	const context = element?.getContext('2d')
	if (!element || !context) return
	const view = new ImageData(SIZE, SIZE)
	for (let i = 0; i < SIZE * SIZE; i++) {
		const id = owner[i]
		if (id === 0 || isHidden(id)) continue
		const factor = isEditable(id) ? 1 : 0.35
		view.data[i * 4] = pixels.data[i * 4]
		view.data[i * 4 + 1] = pixels.data[i * 4 + 1]
		view.data[i * 4 + 2] = pixels.data[i * 4 + 2]
		view.data[i * 4 + 3] = Math.round(pixels.data[i * 4 + 3] * factor)
	}
	context.putImageData(view, 0, 0)
	schedulePreview()
}

let previewTimer: ReturnType<typeof setTimeout> | undefined
function schedulePreview() {
	clearTimeout(previewTimer)
	previewTimer = setTimeout(updatePreview, 120)
}

function updatePreview() {
	// The 3D preview gets the skin without the hidden parts.
	const copy = new ImageData(new Uint8ClampedArray(pixels.data), SIZE, SIZE)
	for (let i = 0; i < SIZE * SIZE; i++) {
		if (isHidden(owner[i])) copy.data[i * 4 + 3] = 0
	}
	const temporary = document.createElement('canvas')
	temporary.width = SIZE
	temporary.height = SIZE
	temporary.getContext('2d')?.putImageData(copy, 0, 0)
	previewUrl.value = temporary.toDataURL('image/png')
}

onBeforeUnmount(() => clearTimeout(previewTimer))

watch([layerMode, hidden], redraw, { deep: true })
watch(variant, () => {
	buildOwners()
	redraw()
})

// ---------- history ----------

function snapshot(): ImageData {
	return new ImageData(new Uint8ClampedArray(pixels.data), SIZE, SIZE)
}

function pushUndo() {
	undoStack.push(snapshot())
	if (undoStack.length > 60) undoStack.shift()
	redoStack = []
	historyVersion.value++
}

function undo() {
	const previous = undoStack.pop()
	if (!previous) return
	redoStack.push(snapshot())
	pixels = previous
	historyVersion.value++
	redraw()
}

function redo() {
	const next = redoStack.pop()
	if (!next) return
	undoStack.push(snapshot())
	pixels = next
	historyVersion.value++
	redraw()
}

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

function cleanup() {
	window.removeEventListener('keydown', onKeydown)
}

onBeforeUnmount(cleanup)

// ---------- tools ----------

function hexToRgb(hex: string): [number, number, number] {
	const value = hex.replace('#', '')
	return [parseInt(value.slice(0, 2), 16) || 0, parseInt(value.slice(2, 4), 16) || 0, parseInt(value.slice(4, 6), 16) || 0]
}

function setPixel(x: number, y: number) {
	if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return
	const index = y * SIZE + x
	const id = owner[index]
	if (!isEditable(id)) return
	const o = index * 4
	if (tool.value === 'eraser') {
		const info = ownerInfo(id)
		if (info?.layer === 1) {
			// Overlay: back to transparent
			pixels.data[o + 3] = 0
		} else {
			// Base layer: back to the original pixel (a transparent base shows up black in game)
			for (let c = 0; c < 4; c++) pixels.data[o + c] = original.data[o + c]
		}
		return
	}
	const [r, g, b] = hexToRgb(color.value)
	pixels.data[o] = r
	pixels.data[o + 1] = g
	pixels.data[o + 2] = b
	// The base layer has to stay opaque
	pixels.data[o + 3] = ownerInfo(id)?.layer === 0 ? 255 : alpha.value
}

function stamp(x: number, y: number) {
	const offset = Math.floor((brushSize.value - 1) / 2)
	for (let dy = 0; dy < brushSize.value; dy++) {
		for (let dx = 0; dx < brushSize.value; dx++) setPixel(x - offset + dx, y - offset + dy)
	}
}

function line(x0: number, y0: number, x1: number, y1: number) {
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
	const startIndex = startY * SIZE + startX
	const startOwner = owner[startIndex]
	if (!isEditable(startOwner)) return
	const target = Array.from(pixels.data.slice(startIndex * 4, startIndex * 4 + 4))
	const [r, g, b] = hexToRgb(color.value)
	const a = ownerInfo(startOwner)?.layer === 0 ? 255 : alpha.value
	if (target[0] === r && target[1] === g && target[2] === b && target[3] === a) return

	const stack = [startX, startY]
	while (stack.length) {
		const y = stack.pop() as number
		const x = stack.pop() as number
		if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) continue
		const index = y * SIZE + x
		if (owner[index] !== startOwner) continue
		const o = index * 4
		if (
			pixels.data[o] !== target[0] ||
			pixels.data[o + 1] !== target[1] ||
			pixels.data[o + 2] !== target[2] ||
			pixels.data[o + 3] !== target[3]
		)
			continue
		pixels.data[o] = r
		pixels.data[o + 1] = g
		pixels.data[o + 2] = b
		pixels.data[o + 3] = a
		stack.push(x + 1, y, x - 1, y, x, y + 1, x, y - 1)
	}
}

function pickColor(x: number, y: number) {
	const index = y * SIZE + x
	if (x < 0 || y < 0 || x >= SIZE || y >= SIZE || owner[index] === 0 || isHidden(owner[index])) return
	const o = index * 4
	if (pixels.data[o + 3] === 0) return
	color.value = '#' + [0, 1, 2].map((c) => pixels.data[o + c].toString(16).padStart(2, '0')).join('')
	alpha.value = pixels.data[o + 3]
	tool.value = 'pencil'
}

let drawing = false
let lastX = 0
let lastY = 0

function pointerPixel(event: PointerEvent): [number, number] {
	const rect = canvas.value!.getBoundingClientRect()
	return [
		Math.floor(((event.clientX - rect.left) * SIZE) / rect.width),
		Math.floor(((event.clientY - rect.top) * SIZE) / rect.height),
	]
}

function onPointerDown(event: PointerEvent) {
	if (event.button !== 0) return
	const [x, y] = pointerPixel(event)
	if (tool.value === 'picker') {
		pickColor(x, y)
		return
	}
	pushUndo()
	canvas.value?.setPointerCapture(event.pointerId)
	if (tool.value === 'fill') {
		if (x >= 0 && y >= 0 && x < SIZE && y < SIZE) floodFill(x, y)
		redraw()
		return
	}
	drawing = true
	lastX = x
	lastY = y
	stamp(x, y)
	redraw()
}

function onPointerMove(event: PointerEvent) {
	const [x, y] = pointerPixel(event)
	const info = x >= 0 && y >= 0 && x < SIZE && y < SIZE ? ownerInfo(owner[y * SIZE + x]) : null
	hover.value = info
		? formatMessage(messages.hover, {
				part: partLabels.value[info.part],
				layer: formatMessage(info.layer === 0 ? messages.layerBase : messages.layerOverlay),
			})
		: ''
	if (!drawing) return
	if (x === lastX && y === lastY) return
	line(lastX, lastY, x, y)
	lastX = x
	lastY = y
	redraw()
}

function onPointerUp() {
	drawing = false
}

// ---------- parts and layers ----------

function toggleHidden(part: PartKey, layer: 0 | 1) {
	const next = new Set(hidden.value)
	const key = `${part}:${layer}`
	if (next.has(key)) next.delete(key)
	else next.add(key)
	hidden.value = next
}

function showAllParts() {
	hidden.value = new Set()
}

function resetLayer() {
	pushUndo()
	for (let i = 0; i < SIZE * SIZE; i++) {
		const info = ownerInfo(owner[i])
		if (!info || !isEditable(owner[i])) continue
		const o = i * 4
		if (info.layer === 1) {
			pixels.data[o + 3] = 0
		} else {
			for (let c = 0; c < 4; c++) pixels.data[o + c] = original.data[o + c]
		}
	}
	redraw()
}

// ---------- open / use ----------

async function loadBase(textureUrl: string | null) {
	const temporary = document.createElement('canvas')
	temporary.width = SIZE
	temporary.height = SIZE
	const context = temporary.getContext('2d', { willReadFrequently: true })
	if (!context) return
	if (textureUrl) {
		try {
			const blob = await (await fetch(textureUrl)).blob()
			const bitmap = await createImageBitmap(blob)
			context.drawImage(bitmap, 0, 0)
			bitmap.close()
		} catch {
			// start with an empty skin
		}
	}
	original = context.getImageData(0, 0, SIZE, SIZE)
	pixels = new ImageData(new Uint8ClampedArray(original.data), SIZE, SIZE)
}

async function show(steveTextureUrl: string | null) {
	variant.value = 'CLASSIC'
	tool.value = 'pencil'
	layerMode.value = 'both'
	hidden.value = new Set()
	zoom.value = 1
	undoStack = []
	redoStack = []
	historyVersion.value++
	buildOwners()
	await loadBase(steveTextureUrl)
	modal.value?.show()
	window.addEventListener('keydown', onKeydown)
	await nextTick()
	redraw()
	updatePreview()
}

async function useSkin() {
	const temporary = document.createElement('canvas')
	temporary.width = SIZE
	temporary.height = SIZE
	temporary.getContext('2d')?.putImageData(pixels, 0, 0)
	const blob = await new Promise<Blob | null>((resolve) => temporary.toBlob(resolve, 'image/png'))
	if (!blob) return
	emit('use', new Uint8Array(await blob.arrayBuffer()))
	modal.value?.hide()
}

defineExpose({ show })

const tools = computed<{ id: Tool; label: string }[]>(() => [
	{ id: 'pencil', label: formatMessage(messages.pencil) },
	{ id: 'eraser', label: formatMessage(messages.eraser) },
	{ id: 'fill', label: formatMessage(messages.fill) },
	{ id: 'picker', label: formatMessage(messages.picker) },
])
const layerModes = computed<{ id: LayerMode; label: string }[]>(() => [
	{ id: 'base', label: formatMessage(messages.layerBase) },
	{ id: 'overlay', label: formatMessage(messages.layerOverlay) },
	{ id: 'both', label: formatMessage(messages.layerBoth) },
])
</script>
<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		width="980px"
		max-width="980px"
		:on-hide="cleanup"
		:close-on-click-outside="false"
	>
		<div class="flex flex-col gap-3">
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
				<span class="mx-1 h-6 w-px bg-surface-5"></span>
				<input v-model="color" type="color" class="h-8 w-10 cursor-pointer rounded border-0 bg-transparent p-0" />
				<label class="flex items-center gap-2 text-sm text-secondary">
					{{ formatMessage(messages.opacity) }}
					<input v-model.number="alpha" type="range" min="1" max="255" class="w-24" />
				</label>
				<label class="flex items-center gap-2 text-sm text-secondary">
					{{ formatMessage(messages.size, { value: brushSize }) }}
					<input v-model.number="brushSize" type="range" min="1" max="4" class="w-20" />
				</label>
				<label class="flex items-center gap-2 text-sm text-secondary">
					{{ formatMessage(messages.zoom) }}
					<input v-model.number="zoom" type="range" min="0.75" max="1.5" step="0.25" class="w-20" />
				</label>
				<label class="flex items-center gap-1 text-sm text-secondary">
					<input v-model="showGrid" type="checkbox" />{{ formatMessage(messages.grid) }}
				</label>
			</div>

			<div class="flex flex-col gap-4 md:flex-row">
				<div class="min-w-0 flex-1">
					<div class="max-h-[540px] overflow-auto rounded-2xl border border-solid border-surface-4 bg-bg-input p-3 shadow-inner">
						<div
							class="relative mx-auto w-fit"
							:style="{
								width: `${SIZE * scale}px`,
								height: `${SIZE * scale}px`,
								backgroundColor: '#1a1a22',
								backgroundImage:
									'conic-gradient(#8884 25%, transparent 0 50%, #8884 0 75%, transparent 0)',
								backgroundSize: '16px 16px',
							}"
						>
							<canvas
								ref="canvas"
								:width="SIZE"
								:height="SIZE"
								class="block touch-none [image-rendering:pixelated]"
								:style="{ width: `${SIZE * scale}px`, height: `${SIZE * scale}px`, cursor: 'crosshair' }"
								@pointerdown="onPointerDown"
								@pointermove="onPointerMove"
								@pointerup="onPointerUp"
								@pointercancel="onPointerUp"
								@pointerleave="hover = ''"
							/>
							<div
								v-if="showGrid"
								class="pointer-events-none absolute inset-0"
								:style="{
									backgroundImage:
										'linear-gradient(to right, rgba(128,128,128,.28) 1px, transparent 1px), linear-gradient(to bottom, rgba(128,128,128,.28) 1px, transparent 1px)',
									backgroundSize: `${scale}px ${scale}px`,
								}"
							></div>
						</div>
					</div>
					<div class="mt-1 h-5 text-sm text-secondary">{{ hover }}</div>
				</div>

				<div class="flex w-full flex-col gap-3 md:w-72">
					<div class="h-[18rem] w-full rounded-2xl bg-surface-2 shadow-inner">
						<SkinPreviewRenderer
							:variant="variant"
							:texture-src="previewUrl"
							framing="modal"
							:initial-rotation="Math.PI / 8"
							class="h-full w-full"
						/>
					</div>

					<div class="flex flex-col gap-1">
						<span class="font-semibold text-contrast">{{ formatMessage(messages.armStyle) }}</span>
						<div class="flex gap-2">
							<Button
								:type="variant === 'CLASSIC' ? 'colored' : 'outlined'"
								:color="variant === 'CLASSIC' ? 'brand' : undefined"
								@click="variant = 'CLASSIC'"
							>
								{{ formatMessage(messages.classic) }}
							</Button>
							<Button
								:type="variant === 'SLIM' ? 'colored' : 'outlined'"
								:color="variant === 'SLIM' ? 'brand' : undefined"
								@click="variant = 'SLIM'"
							>
								{{ formatMessage(messages.slim) }}
							</Button>
						</div>
					</div>

					<div class="flex flex-col gap-1">
						<span class="font-semibold text-contrast">{{ formatMessage(messages.layer) }}</span>
						<div class="flex gap-2">
							<Button
								v-for="entry in layerModes"
								:key="entry.id"
								:type="layerMode === entry.id ? 'colored' : 'outlined'"
								:color="layerMode === entry.id ? 'brand' : undefined"
								@click="layerMode = entry.id"
							>
								{{ entry.label }}
							</Button>
						</div>
						<span class="text-xs text-secondary">{{ formatMessage(messages.layerHint) }}</span>
					</div>

					<div class="flex flex-col gap-1">
						<div class="flex items-center justify-between">
							<span class="font-semibold text-contrast">{{ formatMessage(messages.parts) }}</span>
							<Button type="transparent" @click="showAllParts">{{ formatMessage(messages.showAll) }}</Button>
						</div>
						<div
							v-for="part in PARTS"
							:key="part.key"
							class="flex items-center justify-between gap-2 text-sm"
						>
							<span class="text-primary">{{ partLabels[part.key] }}</span>
							<span class="flex gap-1">
								<button
									v-for="layer in ([0, 1] as const)"
									:key="layer"
									class="flex cursor-pointer items-center gap-1 rounded-xl border border-solid border-surface-5 bg-transparent px-2.5 py-1 text-xs transition-colors text-secondary hover:bg-surface-4"
									:class="{ 'opacity-50': hidden.has(`${part.key}:${layer}`) }"
									@click="toggleHidden(part.key, layer)"
								>
									<EyeOffIcon v-if="hidden.has(`${part.key}:${layer}`)" class="size-4" />
									<EyeIcon v-else class="size-4" />
									{{ formatMessage(layer === 0 ? messages.layerBase : messages.layerOverlay) }}
								</button>
							</span>
						</div>
						<span class="text-xs text-secondary">{{ formatMessage(messages.partsHint) }}</span>
					</div>

					<Button type="outlined" @click="resetLayer">{{ formatMessage(messages.resetLayer) }}</Button>
				</div>
			</div>
		</div>

		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(messages.cancel) }}
				</Button>
				<Button type="colored" color="brand" @click="useSkin">
					{{ formatMessage(messages.use) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
