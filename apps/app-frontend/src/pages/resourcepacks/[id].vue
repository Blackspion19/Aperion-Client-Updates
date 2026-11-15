<script setup lang="ts">
import {
	ArrowLeftIcon,
	DownloadIcon,
	FileImageIcon,
	PackageIcon,
	PauseIcon,
	PlayIcon,
	SearchIcon,
	SpinnerIcon,
	TagCategoryAudioIcon,
	TrashIcon,
	TypeIcon,
	UploadIcon,
} from '@aperion/assets'
import { Button, defineMessages, injectNotificationManager, Input, useVIntl } from '@aperion/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, onMounted, reactive, ref, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AssetThumb from '@/components/ui/resourcepack/AssetThumb.vue'
import DeployToInstanceCard from '@/components/ui/resourcepack/DeployToInstanceCard.vue'
import EditResourcePackDetailsModal from '@/components/ui/resourcepack/EditResourcePackDetailsModal.vue'
import TexturePainterModal from '@/components/ui/resourcepack/TexturePainterModal.vue'
import {
	export_resourcepack,
	get_resourcepack,
	list_vanilla_assets,
	read_resourcepack_asset,
	read_vanilla_asset,
	remove_resourcepack_asset,
	set_resourcepack_icon,
	set_resourcepack_sound,
	set_resourcepack_texture,
	type ResourcePackMetadata,
	type VanillaAsset,
	type VanillaAssetKind,
} from '@/helpers/resourcepack'
import { clearPreviews } from '@/helpers/resourcepack-previews'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()

const packId = computed(() => String(route.params.id ?? ''))

const pack = ref<ResourcePackMetadata>()
const assets = ref<VanillaAsset[]>([])
const loading = ref(true)
const assetsError = ref<string | null>(null)
const activeKind = ref<VanillaAssetKind>('texture')
const search = ref('')

const editDetailsModal = useTemplateRef('editDetailsModal')
const iconFileInput = useTemplateRef('iconFileInput')
const painterModal = useTemplateRef('painterModal')
// One shared file input per kind instead of one per list entry.
const textureFileInput = useTemplateRef<HTMLInputElement>('textureFileInput')
const soundFileInput = useTemplateRef<HTMLInputElement>('soundFileInput')
let pendingPath = ''
// Bumped when the pack's own copy of an asset changes, so thumbnails reload.
const revisions = reactive<Record<string, number>>({})
const busyAssets = ref(new Set<string>())

// The list can contain many thousands of assets. Only a page of them is
// rendered at once so the editor stays fast.
const PAGE_SIZE = 100
const visibleCount = ref(PAGE_SIZE)

const messages = defineMessages({
	back: { id: 'app.resourcepacks.editor.back', defaultMessage: 'Back to resource packs' },
	texturesTab: { id: 'app.resourcepacks.editor.tab.textures', defaultMessage: 'Textures' },
	soundsTab: { id: 'app.resourcepacks.editor.tab.sounds', defaultMessage: 'Sounds' },
	fontsTab: { id: 'app.resourcepacks.editor.tab.fonts', defaultMessage: 'Fonts' },
	searchPlaceholder: {
		id: 'app.resourcepacks.editor.search',
		defaultMessage: 'Search assets…',
	},
	overriddenBadge: {
		id: 'app.resourcepacks.editor.overridden',
		defaultMessage: 'Replaced',
	},
	playSound: { id: 'app.resourcepacks.editor.play-sound', defaultMessage: 'Play sound' },
	stopSound: { id: 'app.resourcepacks.editor.stop-sound', defaultMessage: 'Stop sound' },
	replaceButton: {
		id: 'app.resourcepacks.editor.replace',
		defaultMessage: 'Replace',
	},
	revertButton: {
		id: 'app.resourcepacks.editor.revert',
		defaultMessage: 'Revert to vanilla',
	},
	editDetails: {
		id: 'app.resourcepacks.editor.edit-details',
		defaultMessage: 'Edit details',
	},
	setIcon: {
		id: 'app.resourcepacks.editor.set-icon',
		defaultMessage: 'Set icon',
	},
	assetsUnavailable: {
		id: 'app.resourcepacks.editor.assets-unavailable',
		defaultMessage:
			'The assets of Minecraft {version} could not be downloaded. Check your internet connection and try again.',
	},
	exportButton: {
		id: 'app.resourcepacks.export',
		defaultMessage: 'Export',
	},
	showMore: {
		id: 'app.resourcepacks.editor.show-more',
		defaultMessage: 'Show more ({shown} of {total})',
	},
	noResults: {
		id: 'app.resourcepacks.editor.no-results',
		defaultMessage: 'No assets match your search.',
	},
})

async function refresh() {
	loading.value = true
	assetsError.value = null
	try {
		pack.value = (await get_resourcepack(packId.value)) ?? undefined
		if (!pack.value) {
			router.replace('/resourcepacks')
			return
		}
		try {
			assets.value = await list_vanilla_assets(pack.value.game_version)
		} catch (e) {
			assetsError.value = e instanceof Error ? e.message : String(e)
			assets.value = []
		}
	} catch (e) {
		handleError(e)
	} finally {
		loading.value = false
	}
}

const filteredAssets = computed(() => {
	const query = search.value.trim().toLowerCase()
	return assets.value
		.filter((asset) => asset.kind === activeKind.value)
		.filter((asset) => !query || asset.path.toLowerCase().includes(query))
})

const visibleAssets = computed(() => filteredAssets.value.slice(0, visibleCount.value))

watch([activeKind, search], () => {
	visibleCount.value = PAGE_SIZE
})

async function exportPack() {
	if (!pack.value) return
	try {
		const destination = await save({
			defaultPath: `${pack.value.name}.zip`,
			filters: [{ name: 'Resource pack', extensions: ['zip'] }],
		})
		if (!destination) return
		await export_resourcepack(pack.value.id, destination)
	} catch (e) {
		handleError(e)
	}
}

const overriddenSet = computed(() => new Set(pack.value?.overridden_assets ?? []))

function isOverridden(path: string): boolean {
	return overriddenSet.value.has(path)
}

function assetLabel(path: string): string {
	return path.replace(/^assets\/minecraft\//, '')
}

function openTextureBrowser(path: string) {
	pendingPath = path
	textureFileInput.value?.click()
}

function openSoundBrowser(path: string) {
	pendingPath = path
	soundFileInput.value?.click()
}

async function onTextureFileChange(event: Event) {
	const path = pendingPath
	const input = event.target as HTMLInputElement
	const file = input.files?.[0]
	if (!file || !pack.value) return

	busyAssets.value.add(path)
	try {
		const bytes = new Uint8Array(await file.arrayBuffer())
		pack.value = await set_resourcepack_texture(pack.value.id, path, bytes)
		invalidateOverriddenPreview(path)
	} catch (e) {
		handleError(e)
	} finally {
		busyAssets.value.delete(path)
		input.value = ''
	}
}

async function onSoundFileChange(event: Event) {
	const path = pendingPath
	const input = event.target as HTMLInputElement
	const file = input.files?.[0]
	if (!file || !pack.value) return

	if (playingPath.value === path) stopSound()
	busyAssets.value.add(path)
	try {
		const bytes = new Uint8Array(await file.arrayBuffer())
		pack.value = await set_resourcepack_sound(pack.value.id, path, bytes)
		invalidateOverriddenPreview(path)
	} catch (e) {
		handleError(e)
	} finally {
		busyAssets.value.delete(path)
		input.value = ''
	}
}

async function revertAsset(path: string) {
	if (!pack.value) return
	if (playingPath.value === path) stopSound()
	busyAssets.value.add(path)
	try {
		pack.value = await remove_resourcepack_asset(pack.value.id, path)
		invalidateOverriddenPreview(path)
	} catch (e) {
		handleError(e)
	} finally {
		busyAssets.value.delete(path)
	}
}

function openIconBrowser() {
	iconFileInput.value?.click()
}

async function onIconFileChange(event: Event) {
	const input = event.target as HTMLInputElement
	const file = input.files?.[0]
	if (!file || !pack.value) return

	try {
		const bytes = new Uint8Array(await file.arrayBuffer())
		pack.value = await set_resourcepack_icon(pack.value.id, bytes)
	} catch (e) {
		handleError(e)
	} finally {
		input.value = ''
	}
}

function onDetailsSaved(updated: ResourcePackMetadata) {
	const versionChanged = pack.value?.game_version !== updated.game_version
	pack.value = updated
	if (versionChanged) {
		// Vanilla previews belong to the old version; load everything again.
		clearPreviews('v:')
		visibleCount.value = PAGE_SIZE
		refresh()
	}
}

function invalidateOverriddenPreview(path: string) {
	revisions[path] = (revisions[path] ?? 0) + 1
}

function onPainted(updated: ResourcePackMetadata, path: string) {
	pack.value = updated
	invalidateOverriddenPreview(path)
}

onBeforeUnmount(() => clearPreviews())

/**
 * Opens the editor for exactly the asset that was clicked. This is the fix
 * for the bug in the old launcher, where clicking an already-customized
 * item/block texture opened a generic, unrelated "add texture" picker
 * instead of jumping straight into editing that specific texture — the
 * click handler there didn't carry the clicked asset's path through at
 * all. Here every entry point (the tile itself, and the per-kind buttons)
 * always resolves to the same `asset.path` that was rendered, so replacing
 * an already-overridden texture/sound always edits that exact asset.
 */
// ---------- playing sounds ----------
// Web Audio is used instead of an <audio> element, so no extra permission
// for media files is needed. Only one sound plays at a time.
let audioContext: AudioContext | null = null
let currentSource: AudioBufferSourceNode | null = null
let soundTicket = 0
const playingPath = ref<string | null>(null)
const loadingSoundPath = ref<string | null>(null)

function stopSound() {
	soundTicket++
	if (currentSource) {
		currentSource.onended = null
		try {
			currentSource.stop()
		} catch {
			// already stopped
		}
		currentSource.disconnect()
		currentSource = null
	}
	playingPath.value = null
	loadingSoundPath.value = null
}

async function toggleSound(path: string) {
	if (!pack.value) return
	// A second click on the same sound stops it
	if (playingPath.value === path || loadingSoundPath.value === path) {
		stopSound()
		return
	}
	stopSound()
	const ticket = soundTicket
	loadingSoundPath.value = path
	try {
		// A replaced sound plays the pack's own file, otherwise the original
		const bytes = isOverridden(path)
			? await read_resourcepack_asset(pack.value.id, path)
			: await read_vanilla_asset(pack.value.game_version, path)
		audioContext ??= new AudioContext()
		if (audioContext.state === 'suspended') await audioContext.resume()
		const buffer = await audioContext.decodeAudioData(
			bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer,
		)
		// Something else was started or stopped in the meantime
		if (ticket !== soundTicket) return
		const source = audioContext.createBufferSource()
		source.buffer = buffer
		source.connect(audioContext.destination)
		source.onended = () => {
			if (currentSource === source) stopSound()
		}
		currentSource = source
		source.start()
		playingPath.value = path
	} catch (e) {
		handleError(e)
	} finally {
		if (ticket === soundTicket) loadingSoundPath.value = null
	}
}

onBeforeUnmount(() => {
	stopSound()
	void audioContext?.close()
	audioContext = null
})

function openAssetEditor(asset: VanillaAsset) {
	if (asset.kind === 'texture') {
		painterModal.value?.show(asset.path, isOverridden(asset.path))
	} else if (asset.kind === 'sound') {
		// Clicking a sound plays it; replacing is only done with the Replace button
		void toggleSound(asset.path)
	}
}

onMounted(refresh)
</script>

<template>
	<input ref="iconFileInput" type="file" accept="image/png" class="hidden" @change="onIconFileChange" />
	<input
		ref="textureFileInput"
		type="file"
		accept="image/png"
		class="hidden"
		@change="onTextureFileChange"
	/>
	<input
		ref="soundFileInput"
		type="file"
		accept="audio/ogg,.ogg"
		class="hidden"
		@change="onSoundFileChange"
	/>
	<TexturePainterModal v-if="pack" ref="painterModal" :pack="pack" @saved="onPainted" />
	<EditResourcePackDetailsModal
		v-if="pack"
		ref="editDetailsModal"
		:pack="pack"
		@saved="onDetailsSaved"
	/>

	<div v-if="pack" class="flex flex-col gap-4">
		<Button type="transparent" class="w-fit !px-0" @click="router.push('/resourcepacks')">
			<ArrowLeftIcon />
			{{ formatMessage(messages.back) }}
		</Button>

		<div class="flex flex-wrap items-center gap-4 rounded-2xl border border-solid border-surface-4 bg-bg-raised p-4">
			<button
				class="group relative flex size-16 shrink-0 items-center justify-center rounded-xl border border-dashed border-surface-5 bg-bg-input"
				@click="openIconBrowser"
			>
				<img
					v-if="pack.icon_path"
					:src="convertFileSrc(pack.icon_path)"
					class="size-full rounded-xl object-cover"
					alt=""
				/>
				<PackageIcon v-else class="size-8 text-secondary" />
				<span
					class="absolute inset-0 hidden items-center justify-center rounded-xl bg-black/60 text-white group-hover:flex"
				>
					<UploadIcon class="size-5" />
				</span>
			</button>
			<div class="flex min-w-0 flex-1 flex-col">
				<h1 class="m-0 truncate text-xl font-extrabold text-contrast">{{ pack.name }}</h1>
				<span class="text-sm text-secondary">
					{{ pack.game_version
					}}{{
						pack.max_game_version === '*'
							? '+'
							: pack.max_game_version
								? ` – ${pack.max_game_version}`
								: ''
					}} · pack_format {{ pack.pack_format }}
				</span>
				<p v-if="pack.description" class="m-0 mt-1 text-sm text-secondary">
					{{ pack.description }}
				</p>
			</div>
			<Button type="outlined" @click="exportPack">
				<DownloadIcon />
				{{ formatMessage(messages.exportButton) }}
			</Button>
			<Button type="outlined" @click="editDetailsModal?.show()">
				{{ formatMessage(messages.editDetails) }}
			</Button>
		</div>

		<DeployToInstanceCard :pack="pack" />

		<div class="flex flex-wrap items-center gap-2">
			<Button
				:type="activeKind === 'texture' ? 'colored' : 'outlined'"
				:color="activeKind === 'texture' ? 'brand' : undefined"
				@click="activeKind = 'texture'"
			>
				<FileImageIcon />
				{{ formatMessage(messages.texturesTab) }}
			</Button>
			<Button
				:type="activeKind === 'sound' ? 'colored' : 'outlined'"
				:color="activeKind === 'sound' ? 'brand' : undefined"
				@click="activeKind = 'sound'"
			>
				<TagCategoryAudioIcon />
				{{ formatMessage(messages.soundsTab) }}
			</Button>
			<Button
				:type="activeKind === 'font' ? 'colored' : 'outlined'"
				:color="activeKind === 'font' ? 'brand' : undefined"
				@click="activeKind = 'font'"
			>
				<TypeIcon />
				{{ formatMessage(messages.fontsTab) }}
			</Button>
			<Input
				v-model="search"
				:icon="SearchIcon"
				type="text"
				autocomplete="off"
				:spellcheck="false"
				wrapper-class="flex-1 min-w-[12rem]"
				clearable
				:placeholder="formatMessage(messages.searchPlaceholder)"
			/>
		</div>

		<p v-if="assetsError" class="m-0 rounded-lg bg-bg-orange p-3 text-sm text-orange">
			{{ formatMessage(messages.assetsUnavailable, { version: pack.game_version }) }}
		</p>

		<p v-else-if="!loading && filteredAssets.length === 0" class="m-0 text-sm text-secondary">
			{{ formatMessage(messages.noResults) }}
		</p>

		<div v-else class="flex flex-col gap-2">
			<div
				v-for="asset in visibleAssets"
				:key="asset.path"
				role="button"
				tabindex="0"
				class="ap-lift flex cursor-pointer items-center gap-3 rounded-2xl border border-solid border-surface-4 bg-bg-raised p-3 hover:border-brand focus-visible:border-brand focus-visible:outline-none"
				:class="playingPath === asset.path ? '!border-brand' : ''"
				@click="openAssetEditor(asset)"
				@keydown.enter="openAssetEditor(asset)"
				@keydown.space.prevent="openAssetEditor(asset)"
			>
				<AssetThumb
					v-if="asset.kind === 'texture'"
					:game-version="pack.game_version"
					:pack-id="pack.id"
					:path="asset.path"
					:overridden="isOverridden(asset.path)"
					:revision="revisions[asset.path] ?? 0"
				/>
				<button
					v-else-if="asset.kind === 'sound'"
					class="flex size-10 shrink-0 cursor-pointer items-center justify-center rounded-full border-0 bg-surface-4 text-contrast transition-colors hover:bg-brand hover:text-accent-contrast"
					:class="playingPath === asset.path ? '!bg-brand !text-accent-contrast' : ''"
					:aria-label="formatMessage(playingPath === asset.path ? messages.stopSound : messages.playSound)"
					@click.stop="toggleSound(asset.path)"
				>
					<SpinnerIcon v-if="loadingSoundPath === asset.path" class="size-5 animate-spin" />
					<PauseIcon v-else-if="playingPath === asset.path" class="size-5" />
					<PlayIcon v-else class="size-5" />
				</button>
				<TypeIcon v-else class="size-10 shrink-0 text-secondary" />

				<div class="flex min-w-0 flex-1 flex-col">
					<span class="truncate font-mono text-sm text-contrast">{{ assetLabel(asset.path) }}</span>
					<span v-if="isOverridden(asset.path)" class="w-fit rounded-full bg-bg-green px-2 text-xs text-green">
						{{ formatMessage(messages.overriddenBadge) }}
					</span>
				</div>

				<template v-if="asset.kind === 'texture'">
					<Button
						type="outlined"
						:disabled="busyAssets.has(asset.path)"
						@click.stop="openTextureBrowser(asset.path)"
					>
						<UploadIcon />
						{{ formatMessage(messages.replaceButton) }}
					</Button>
				</template>
				<template v-else-if="asset.kind === 'sound'">
					<Button
						type="outlined"
						:disabled="busyAssets.has(asset.path)"
						@click.stop="openSoundBrowser(asset.path)"
					>
						<UploadIcon />
						{{ formatMessage(messages.replaceButton) }}
					</Button>
				</template>

				<Button
					v-if="isOverridden(asset.path)"
					type="outlined"
					color="red"
					:disabled="busyAssets.has(asset.path)"
					@click.stop="revertAsset(asset.path)"
				>
					<TrashIcon />
				</Button>
			</div>
			<Button
				v-if="filteredAssets.length > visibleCount"
				type="outlined"
				class="w-fit self-center"
				@click="visibleCount += PAGE_SIZE"
			>
				{{
					formatMessage(messages.showMore, {
						shown: visibleCount,
						total: filteredAssets.length,
					})
				}}
			</Button>
		</div>
	</div>
</template>
