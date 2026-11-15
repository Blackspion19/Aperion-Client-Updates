<script setup lang="ts">
import { defineMessages, useVIntl } from '@aperion/ui'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { onBeforeUnmount, onMounted, ref, toRaw, watch } from 'vue'

import { isAirLike, type ParsedSchematic } from '@/helpers/litematic'
import { type Atlas, buildAtlas } from '@/helpers/schematic-atlas'
import type { MeshResult } from '@/helpers/schematic-types'

const props = defineProps<{
	schematic: ParsedSchematic | null
	gameVersion: string
	/** Instance whose mods provide the textures of modded blocks */
	instanceId?: string
	/** Identifies the schematic (and its version) so a built preview can be reused */
	cacheKey?: string
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	textures: { id: 'app.schematics.preview.textures', defaultMessage: 'Loading textures...' },
	meshing: { id: 'app.schematics.preview.meshing', defaultMessage: 'Building preview...' },
	tooLarge: {
		id: 'app.schematics.preview.too-large',
		defaultMessage: 'This schematic is too large for the preview.',
	},
	reduced: {
		id: 'app.schematics.preview.reduced',
		defaultMessage: 'Large schematic: shown in reduced detail ({step}×)',
	},
	truncated: {
		id: 'app.schematics.preview.truncated',
		defaultMessage: 'Only part of the blocks is shown.',
	},
	failed: { id: 'app.schematics.preview.failed', defaultMessage: 'The preview could not be built.' },
	empty: { id: 'app.schematics.preview.empty', defaultMessage: 'This schematic contains no blocks.' },
	hint: { id: 'app.schematics.preview.hint', defaultMessage: 'Drag to rotate, scroll to zoom' },
})

// Largest grid that is built; bigger schematics are made coarser while reading
const MAX_CELLS = 30_000_000
const MAX_FACES = 2_000_000

type Status = 'idle' | 'textures' | 'meshing' | 'ready' | 'too-large' | 'empty' | 'failed'

const container = ref<HTMLDivElement>()
const status = ref<Status>('idle')
const truncated = ref(false)
const detailStep = ref(1)
const errorMessage = ref('')

let renderer: THREE.WebGLRenderer | null = null
let scene: THREE.Scene | null = null
let camera: THREE.PerspectiveCamera | null = null
let controls: OrbitControls | null = null
let resizeObserver: ResizeObserver | null = null
let content: THREE.Group | null = null
let buildTicket = 0
let cancelWorker: (() => void) | null = null

// Built previews are kept, so selecting a schematic again shows it instantly.
interface BuiltPreview {
	mesh: MeshResult
	atlas: Atlas
}
const builtCache = new Map<string, BuiltPreview>()
const CACHE_SIZE = 6

function render() {
	if (renderer && scene && camera) renderer.render(scene, camera)
}

function resize() {
	const element = container.value
	if (!element || !renderer || !camera) return
	const width = Math.max(1, element.clientWidth)
	const height = Math.max(1, element.clientHeight)
	renderer.setSize(width, height, false)
	camera.aspect = width / height
	camera.updateProjectionMatrix()
	render()
}

function setup() {
	const element = container.value
	if (!element || renderer) return
	renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
	renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
	renderer.domElement.style.width = '100%'
	renderer.domElement.style.height = '100%'
	renderer.domElement.style.display = 'block'
	element.appendChild(renderer.domElement)

	scene = new THREE.Scene()
	camera = new THREE.PerspectiveCamera(45, 1, 0.1, 5000)

	controls = new OrbitControls(camera, renderer.domElement)
	controls.enableDamping = false
	controls.addEventListener('change', render)

	resizeObserver = new ResizeObserver(resize)
	resizeObserver.observe(element)
	resize()
}

function clearContent() {
	if (!content || !scene) return
	scene.remove(content)
	const textures = new Set<THREE.Texture>()
	content.traverse((object) => {
		if (object instanceof THREE.Mesh) {
			object.geometry.dispose()
			const material = object.material as THREE.MeshBasicMaterial
			if (material.map) textures.add(material.map)
			material.dispose()
		}
	})
	textures.forEach((texture) => texture.dispose())
	content = null
}

// ---------- building ----------

type WorkerOutput =
	| { type: 'done'; result: MeshResult }
	| { type: 'empty' | 'too-large' }
	| { type: 'failed'; message: string }

function runMesher(schematic: ParsedSchematic, atlas: Atlas): Promise<WorkerOutput | 'cancelled'> {
	cancelWorker?.()
	return new Promise((resolve, reject) => {
		const worker = new Worker(new URL('../../../helpers/schematic-mesher.worker.ts', import.meta.url), {
			type: 'module',
		})
		const finish = (value: WorkerOutput | 'cancelled') => {
			worker.terminate()
			if (cancelWorker === cancel) cancelWorker = null
			resolve(value)
		}
		const cancel = () => finish('cancelled')
		cancelWorker = cancel
		worker.onmessage = (event: MessageEvent<WorkerOutput>) => finish(event.data)
		worker.onerror = (event) => {
			worker.terminate()
			if (cancelWorker === cancel) cancelWorker = null
			reject(new Error(event.message))
		}
		worker.postMessage({
			// A worker cannot receive Vue proxies, so the plain objects are sent
			regions: toRaw(schematic).regions.map((region) => toRaw(region)),
			blocks: atlas.blocks,
			maxCells: MAX_CELLS,
			maxFaces: MAX_FACES,
		})
	})
}

function showBuilt(entry: BuiltPreview) {
	if (!scene) return
	const texture = new THREE.CanvasTexture(entry.atlas.canvas)
	texture.magFilter = THREE.NearestFilter
	texture.minFilter = THREE.NearestFilter
	texture.generateMipmaps = false
	texture.colorSpace = THREE.SRGBColorSpace

	// opaque, cutout (see-through pixels cut away), translucent
	const materials = [
		new THREE.MeshBasicMaterial({ map: texture, vertexColors: true }),
		new THREE.MeshBasicMaterial({ map: texture, vertexColors: true, alphaTest: 0.5 }),
		new THREE.MeshBasicMaterial({
			map: texture,
			vertexColors: true,
			transparent: true,
			opacity: 0.75,
			depthWrite: false,
		}),
	]

	const group = new THREE.Group()
	entry.mesh.layers.forEach((layer, index) => {
		if (layer.indices.length === 0) return
		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute('position', new THREE.BufferAttribute(layer.positions, 3))
		geometry.setAttribute('uv', new THREE.BufferAttribute(layer.uvs, 2))
		geometry.setAttribute('color', new THREE.BufferAttribute(layer.colors, 4, true))
		geometry.setIndex(new THREE.BufferAttribute(layer.indices, 1))
		geometry.computeBoundingSphere()
		const mesh = new THREE.Mesh(geometry, materials[index])
		mesh.renderOrder = index
		group.add(mesh)
	})

	const { x, y, z } = entry.mesh.size
	group.position.set(-x / 2, -y / 2, -z / 2)
	content = group
	scene.add(group)

	// Frame the whole building.
	if (camera && controls) {
		const distance = Math.max(x, y, z) * 1.7 + 4
		camera.position.set(distance * 0.8, distance * 0.6, distance * 0.9)
		camera.near = 0.1
		camera.far = distance * 10 + 1000
		camera.updateProjectionMatrix()
		controls.target.set(0, 0, 0)
		controls.update()
	}
	truncated.value = entry.mesh.truncated
	detailStep.value = entry.mesh.step
	status.value = 'ready'
	render()
}

function blockNames(schematic: ParsedSchematic): string[] {
	const names = new Set<string>()
	for (const region of schematic.regions) {
		for (const state of region.palette) if (!isAirLike(state.name)) names.add(state.name)
	}
	return [...names]
}

async function build(schematic: ParsedSchematic) {
	const ticket = ++buildTicket
	truncated.value = false
	errorMessage.value = ''
	clearContent()
	render()

	try {
		const key = props.cacheKey ? `${props.gameVersion}|${props.cacheKey}` : null
		let entry = key ? builtCache.get(key) : undefined

		if (!entry) {
			status.value = 'textures'
			const atlas = await buildAtlas(props.gameVersion, blockNames(schematic), props.instanceId)
			if (ticket !== buildTicket) return

			status.value = 'meshing'
			const output = await runMesher(schematic, atlas)
			if (ticket !== buildTicket || output === 'cancelled') return
			if (output.type === 'empty' || output.type === 'too-large') {
				status.value = output.type
				return
			}
			if (output.type === 'failed') {
				console.error('Schematic preview: the mesh worker failed:', output.message)
				errorMessage.value = output.message
				status.value = 'failed'
				return
			}
			entry = { mesh: output.result, atlas }
			if (key) {
				builtCache.set(key, entry)
				if (builtCache.size > CACHE_SIZE) builtCache.delete(builtCache.keys().next().value as string)
			}
		}
		showBuilt(entry)
	} catch (error) {
		console.error('Schematic preview failed:', error)
		errorMessage.value = error instanceof Error ? error.message : String(error)
		if (ticket === buildTicket) status.value = 'failed'
	}
}

watch(
	() => props.schematic,
	(schematic) => {
		if (!schematic) {
			buildTicket++
			cancelWorker?.()
			clearContent()
			status.value = 'idle'
			render()
			return
		}
		setup()
		void build(schematic)
	},
)

onMounted(() => {
	setup()
	if (props.schematic) void build(props.schematic)
})

onBeforeUnmount(() => {
	buildTicket++
	cancelWorker?.()
	resizeObserver?.disconnect()
	clearContent()
	controls?.dispose()
	renderer?.dispose()
	renderer?.domElement.remove()
	renderer = null
})
</script>
<template>
	<div class="relative h-full w-full overflow-hidden rounded-xl bg-surface-2">
		<div ref="container" class="h-full w-full"></div>
		<div
			v-if="status !== 'ready'"
			class="pointer-events-none absolute inset-0 flex items-center justify-center p-4 text-center text-secondary"
		>
			<span v-if="status === 'textures'">{{ formatMessage(messages.textures) }}</span>
			<span v-else-if="status === 'meshing'">{{ formatMessage(messages.meshing) }}</span>
			<span v-else-if="status === 'too-large'">{{ formatMessage(messages.tooLarge) }}</span>
			<span v-else-if="status === 'empty'">{{ formatMessage(messages.empty) }}</span>
			<span v-else-if="status === 'failed'">
				{{ formatMessage(messages.failed) }}
				<span v-if="errorMessage" class="mt-1 block break-words text-xs opacity-70">{{ errorMessage }}</span>
			</span>
		</div>
		<div
			v-if="status === 'ready'"
			class="pointer-events-none absolute bottom-2 left-0 right-0 text-center text-xs text-secondary"
		>
			{{ formatMessage(messages.hint) }}
			<span v-if="detailStep > 1"> · {{ formatMessage(messages.reduced, { step: detailStep }) }}</span>
			<span v-if="truncated"> · {{ formatMessage(messages.truncated) }}</span>
		</div>
	</div>
</template>
