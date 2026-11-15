<script setup lang="ts">
import {
	CopyIcon,
	DownloadIcon,
	EditIcon,
	LayersIcon,
	PackageIcon,
	PlusIcon,
	TrashIcon,
	UploadIcon,
} from '@aperion/assets'
import { Button, commonMessages, defineMessages, injectNotificationManager, useVIntl } from '@aperion/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { onMounted, ref, useTemplateRef } from 'vue'
import { useRouter } from 'vue-router'

import {
	delete_resourcepack,
	duplicate_resourcepack,
	export_resourcepack,
	list_resourcepacks,
	type ResourcePackMetadata,
} from '@/helpers/resourcepack'

import ConfirmDeleteResourcePackModal from './ConfirmDeleteResourcePackModal.vue'
import CreateResourcePackModal from '../resourcepack/CreateResourcePackModal.vue'
import ImportResourcePackModal from '../resourcepack/ImportResourcePackModal.vue'
import PackCardBanner from '../resourcepack/PackCardBanner.vue'

defineProps<{
	showHeading?: boolean
}>()

const { formatMessage } = useVIntl()
const router = useRouter()
const { handleError } = injectNotificationManager()

const packs = ref<ResourcePackMetadata[]>([])
const loading = ref(true)

const createModal = useTemplateRef('createModal')
const deleteModal = useTemplateRef('deleteModal')
const importModal = useTemplateRef('importModal')

const messages = defineMessages({
	heading: {
		id: 'app.resourcepacks.heading',
		defaultMessage: 'Resource packs',
	},
	newPack: {
		id: 'app.resourcepacks.new',
		defaultMessage: 'New resource pack',
	},
	emptyTitle: {
		id: 'app.resourcepacks.empty.title',
		defaultMessage: 'No resource packs yet',
	},
	emptyDescription: {
		id: 'app.resourcepacks.empty.description',
		defaultMessage: 'Create a resource pack to start replacing textures and sounds.',
	},
	importPack: {
		id: 'app.resourcepacks.import',
		defaultMessage: 'Import',
	},
	duplicate: {
		id: 'app.resourcepacks.duplicate',
		defaultMessage: 'Duplicate',
	},
	export: {
		id: 'app.resourcepacks.export',
		defaultMessage: 'Export',
	},
	texturesCount: {
		id: 'app.resourcepacks.card.textures-count',
		defaultMessage: '{count, plural, one {# texture} other {# textures}}',
	},
	soundsCount: {
		id: 'app.resourcepacks.card.sounds-count',
		defaultMessage: '{count, plural, one {# sound} other {# sounds}}',
	},
	noChanges: {
		id: 'app.resourcepacks.card.no-changes',
		defaultMessage: 'No changes yet',
	},
	noDescription: {
		id: 'app.resourcepacks.card.no-description',
		defaultMessage: 'No description',
	},
	assetsCount: {
		id: 'app.resourcepacks.card.assets-count',
		defaultMessage:
			'{count, plural, =0 {No overrides} one {# override} other {# overrides}}',
	},
})

async function refresh() {
	loading.value = true
	try {
		packs.value = await list_resourcepacks()
	} catch (e) {
		handleError(e)
	} finally {
		loading.value = false
	}
}

// "1.21" for one version, "1.21+" for "and newer", "1.20.4 – 1.21.1" for a range
function versionRange(pack: ResourcePackMetadata): string {
	if (!pack.max_game_version) return pack.game_version
	if (pack.max_game_version === '*') return `${pack.game_version}+`
	return `${pack.game_version} – ${pack.max_game_version}`
}

function countAssets(pack: ResourcePackMetadata) {
	const textures = pack.overridden_assets.filter((path) => path.endsWith('.png')).length
	const sounds = pack.overridden_assets.filter((path) => path.endsWith('.ogg')).length
	return { textures, sounds }
}

function openEditor(pack: ResourcePackMetadata) {
	router.push(`/resourcepacks/${pack.id}`)
}

function onCreated(pack: ResourcePackMetadata) {
	packs.value.unshift(pack)
	openEditor(pack)
}

async function duplicatePack(pack: ResourcePackMetadata) {
	try {
		const copy = await duplicate_resourcepack(pack.id)
		packs.value.unshift(copy)
	} catch (e) {
		handleError(e)
	}
}

async function exportPack(pack: ResourcePackMetadata) {
	try {
		const destination = await save({
			defaultPath: `${pack.name}.zip`,
			filters: [{ name: 'Resource pack', extensions: ['zip'] }],
		})
		if (!destination) return
		await export_resourcepack(pack.id, destination)
	} catch (e) {
		handleError(e)
	}
}

function confirmDelete(pack: ResourcePackMetadata) {
	deleteModal.value?.show(pack)
}

async function onDeleted(pack: ResourcePackMetadata) {
	try {
		await delete_resourcepack(pack.id)
		packs.value = packs.value.filter((p) => p.id !== pack.id)
	} catch (e) {
		handleError(e)
	}
}

onMounted(refresh)
</script>

<template>
	<CreateResourcePackModal ref="createModal" @created="onCreated" />
	<ImportResourcePackModal ref="importModal" @imported="onCreated" />
	<ConfirmDeleteResourcePackModal ref="deleteModal" @confirm="onDeleted" />

	<div class="flex flex-col gap-4">
		<div v-if="showHeading" class="flex items-center justify-between gap-2">
			<h1 class="m-0 flex items-center gap-2 text-2xl font-extrabold text-contrast">
				<LayersIcon class="size-6" />
				{{ formatMessage(messages.heading) }}
			</h1>
			<div class="flex gap-2">
				<Button type="outlined" @click="importModal?.show()">
					<UploadIcon />
					{{ formatMessage(messages.importPack) }}
				</Button>
				<Button type="colored" color="brand" @click="createModal?.show()">
					<PlusIcon />
					{{ formatMessage(messages.newPack) }}
				</Button>
			</div>
		</div>

		<div v-if="!loading && packs.length === 0" class="flex flex-col items-center gap-3 py-16">
			<PackageIcon class="size-12 text-secondary" />
			<span class="text-lg font-semibold text-contrast">{{
				formatMessage(messages.emptyTitle)
			}}</span>
			<span class="text-secondary">{{ formatMessage(messages.emptyDescription) }}</span>
			<div class="flex gap-2">
				<Button type="outlined" @click="importModal?.show()">
					<UploadIcon />
					{{ formatMessage(messages.importPack) }}
				</Button>
				<Button type="colored" color="brand" @click="createModal?.show()">
					<PlusIcon />
					{{ formatMessage(messages.newPack) }}
				</Button>
			</div>
		</div>

		<div
			v-else
			class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5"
		>
			<div
				v-for="pack in packs"
				:key="pack.id"
				role="button"
				tabindex="0"
				class="ap-lift group/pack flex cursor-pointer flex-col overflow-hidden rounded-[24px] border border-solid border-surface-4 bg-surface-3 outline-none focus-visible:border-brand"
				@click="openEditor(pack)"
				@keydown.enter="openEditor(pack)"
			>
				<PackCardBanner :pack="pack">
					<span
						class="absolute left-3 top-3 rounded-full bg-black/40 px-2.5 py-1 text-xs font-semibold text-white backdrop-blur"
					>
						{{ versionRange(pack) }}
					</span>

					<div
						class="absolute right-2.5 top-2.5 flex gap-1.5 opacity-0 transition-opacity focus-within:opacity-100 group-hover/pack:opacity-100"
					>
						<button
							class="flex size-8 cursor-pointer items-center justify-center rounded-full border-0 bg-black/50 text-white backdrop-blur transition-colors hover:bg-black/75"
							:aria-label="formatMessage(messages.duplicate)"
							@click.stop="duplicatePack(pack)"
						>
							<CopyIcon class="size-4" />
						</button>
						<button
							class="flex size-8 cursor-pointer items-center justify-center rounded-full border-0 bg-black/50 text-white backdrop-blur transition-colors hover:bg-black/75"
							:aria-label="formatMessage(messages.export)"
							@click.stop="exportPack(pack)"
						>
							<DownloadIcon class="size-4" />
						</button>
						<button
							class="flex size-8 cursor-pointer items-center justify-center rounded-full border-0 bg-black/50 text-white backdrop-blur transition-colors hover:bg-red/80"
							:aria-label="formatMessage(commonMessages.deleteLabel)"
							@click.stop="confirmDelete(pack)"
						>
							<TrashIcon class="size-4" />
						</button>
					</div>
				</PackCardBanner>

				<div class="flex flex-1 flex-col gap-3 p-4">
					<div class="min-w-0">
						<h3 class="m-0 truncate text-lg font-bold text-contrast">{{ pack.name }}</h3>
						<p class="m-0 mt-0.5 line-clamp-2 min-h-[2.5rem] text-sm text-secondary">
							{{ pack.description || formatMessage(messages.noDescription) }}
						</p>
					</div>

					<div class="flex flex-wrap gap-1.5">
						<span
							v-if="pack.overridden_assets.length === 0"
							class="rounded-full bg-surface-4 px-2.5 py-1 text-xs font-semibold text-secondary"
						>
							{{ formatMessage(messages.noChanges) }}
						</span>
						<span
							v-if="countAssets(pack).textures > 0"
							class="rounded-full bg-brand-highlight px-2.5 py-1 text-xs font-semibold text-brand"
						>
							{{ formatMessage(messages.texturesCount, { count: countAssets(pack).textures }) }}
						</span>
						<span
							v-if="countAssets(pack).sounds > 0"
							class="rounded-full bg-surface-4 px-2.5 py-1 text-xs font-semibold text-primary"
						>
							{{ formatMessage(messages.soundsCount, { count: countAssets(pack).sounds }) }}
						</span>
					</div>

					<Button class="mt-auto w-full" type="outlined" @click.stop="openEditor(pack)">
						<EditIcon />
						{{ formatMessage(commonMessages.editButton) }}
					</Button>
				</div>
			</div>
		</div>
	</div>
</template>
