<script setup lang="ts">
import {
	ArrowLeftIcon,
	ChevronDownIcon,
	ChevronRightIcon,
	FolderOpenIcon,
	ListIcon,
	PlusIcon,
	TrashIcon,
	UploadIcon,
} from '@aperion/assets'
import {
	Button,
	ConfirmModal,
	defineMessages,
	injectNotificationManager,
	Toggle,
	useVIntl,
} from '@aperion/ui'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import MaterialListModal from '@/components/ui/schematics/MaterialListModal.vue'
import SchematicPreview3D from '@/components/ui/schematics/SchematicPreview3D.vue'
import { get as getInstance, get_full_path } from '@/helpers/instance'
import { isReadableSchematic, type ParsedSchematic, parseSchematicFile } from '@/helpers/litematic'
import {
	type SchematicFile,
	schematics_create_folder,
	schematics_delete,
	schematics_import,
	schematics_list_files,
	schematics_list_folders,
	schematics_read,
} from '@/helpers/schematics'
import type { GameInstance } from '@/helpers/types'

defineOptions({ name: 'SchematicsPage' })

const { formatMessage, locale } = useVIntl()
const { handleError } = injectNotificationManager()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	title: { id: 'app.schematics.title', defaultMessage: 'Schematics' },
	back: { id: 'app.schematics.back', defaultMessage: 'Back to instance' },
	preview: { id: 'app.schematics.preview', defaultMessage: '3D preview' },
	folders: { id: 'app.schematics.folders', defaultMessage: 'Folders' },
	root: { id: 'app.schematics.root', defaultMessage: 'All schematics' },
	newFolder: { id: 'app.schematics.new-folder', defaultMessage: 'New folder' },
	folderName: { id: 'app.schematics.folder-name', defaultMessage: 'Folder name' },
	create: { id: 'app.schematics.create', defaultMessage: 'Create' },
	addFiles: { id: 'app.schematics.add-files', defaultMessage: 'Add schematics' },
	openFolder: { id: 'app.schematics.open-folder', defaultMessage: 'Open folder' },
	empty: {
		id: 'app.schematics.empty',
		defaultMessage: 'No schematics here yet. Drop files onto this page to add them.',
	},
	drop: { id: 'app.schematics.drop', defaultMessage: 'Drop schematics to add them' },
	selectOne: {
		id: 'app.schematics.select-one',
		defaultMessage: 'Select a schematic to see its details.',
	},
	deleteFile: { id: 'app.schematics.delete', defaultMessage: 'Delete' },
	deleteFolder: { id: 'app.schematics.delete-folder', defaultMessage: 'Delete folder' },
	deleteTitle: { id: 'app.schematics.delete.title', defaultMessage: 'Delete this?' },
	deleteDescription: {
		id: 'app.schematics.delete.description',
		defaultMessage: '"{name}" will be deleted permanently.',
	},
	cannotRead: {
		id: 'app.schematics.cannot-read',
		defaultMessage: 'The details of this schematic could not be read.',
	},
	noDetails: {
		id: 'app.schematics.no-details',
		defaultMessage:
			'Details and preview are available for .litematic, .schem and .nbt files.',
	},
	materialList: { id: 'app.schematics.material-list', defaultMessage: 'Material list' },
	loadingDetails: { id: 'app.schematics.loading-details', defaultMessage: 'Reading schematic...' },
	infoName: { id: 'app.schematics.info.name', defaultMessage: 'Name' },
	infoAuthor: { id: 'app.schematics.info.author', defaultMessage: 'Schematic by' },
	infoCreated: { id: 'app.schematics.info.created', defaultMessage: 'Created' },
	infoModified: { id: 'app.schematics.info.modified', defaultMessage: 'Modified' },
	infoRegions: { id: 'app.schematics.info.regions', defaultMessage: 'Regions' },
	infoVolume: { id: 'app.schematics.info.volume', defaultMessage: 'Total volume' },
	infoBlocks: { id: 'app.schematics.info.blocks', defaultMessage: 'Total blocks' },
	infoSize: { id: 'app.schematics.info.size', defaultMessage: 'Enclosing size' },
	infoVersion: { id: 'app.schematics.info.version', defaultMessage: 'Litematic version' },
	infoSchemVersion: { id: 'app.schematics.info.schem-version', defaultMessage: 'Schematic version' },
	infoFormat: { id: 'app.schematics.info.format', defaultMessage: 'Format' },
	formatSponge: { id: 'app.schematics.format.sponge', defaultMessage: 'Sponge schematic (.schem)' },
	formatStructure: { id: 'app.schematics.format.structure', defaultMessage: 'Structure (.nbt)' },
	formatLitematic: { id: 'app.schematics.format.litematic', defaultMessage: 'Litematica (.litematic)' },
	infoMinecraft: { id: 'app.schematics.info.minecraft', defaultMessage: 'Minecraft' },
	infoDescription: { id: 'app.schematics.info.description', defaultMessage: 'Description' },
	infoFile: { id: 'app.schematics.info.file', defaultMessage: 'File size' },
	blocksUnit: { id: 'app.schematics.info.blocks-unit', defaultMessage: '{count} blocks' },
	schema: { id: 'app.schematics.info.schema', defaultMessage: 'Schema {version}' },
})

const instanceId = computed(() => String(route.params.id))
const instance = ref<GameInstance | null>(null)
const instanceDir = ref('')

const folders = ref<string[]>([])
const files = ref<SchematicFile[]>([])
const currentFolder = ref('')
const expanded = ref(new Set<string>())
const selected = ref<SchematicFile | null>(null)
// shallowRef: the block data is large and is sent to a worker, which cannot take Vue proxies
const parsed = shallowRef<ParsedSchematic | null>(null)
const parsing = ref(false)
const parseFailed = ref(false)
const dragging = ref(false)
const creatingFolder = ref(false)
const newFolderName = ref('')
const pendingDelete = ref<{ path: string; name: string } | null>(null)
const deleteModal = useTemplateRef('deleteModal')
const materialModal = useTemplateRef('materialModal')

const PREVIEW_KEY = 'aperion-schematics-preview'
const showPreview = ref(readPreviewSetting())

function readPreviewSetting(): boolean {
	try {
		return window.localStorage.getItem(PREVIEW_KEY) === 'true'
	} catch {
		return false
	}
}

watch(showPreview, (value) => {
	try {
		window.localStorage.setItem(PREVIEW_KEY, String(value))
	} catch {
		// storage blocked
	}
})

// ---------- folder tree ----------

interface FolderRow {
	path: string
	name: string
	depth: number
	hasChildren: boolean
}

const folderRows = computed<FolderRow[]>(() => {
	const rows: FolderRow[] = []
	for (const path of folders.value) {
		const parts = path.split('/')
		// Hidden while any parent folder is collapsed
		let visible = true
		for (let i = 1; i < parts.length; i++) {
			if (!expanded.value.has(parts.slice(0, i).join('/'))) {
				visible = false
				break
			}
		}
		if (!visible) continue
		rows.push({
			path,
			name: parts[parts.length - 1],
			depth: parts.length - 1,
			hasChildren: folders.value.some((other) => other.startsWith(`${path}/`)),
		})
	}
	return rows
})

function toggleExpanded(path: string) {
	const next = new Set(expanded.value)
	if (next.has(path)) next.delete(path)
	else next.add(path)
	expanded.value = next
}

async function refreshFolders() {
	folders.value = await schematics_list_folders(instanceDir.value)
}

async function refreshFiles() {
	files.value = await schematics_list_files(instanceDir.value, currentFolder.value)
	if (selected.value && !files.value.some((file) => file.path === selected.value?.path)) {
		selected.value = null
		parsed.value = null
	}
}

async function selectFolder(path: string) {
	currentFolder.value = path
	selected.value = null
	parsed.value = null
	try {
		await refreshFiles()
	} catch (e) {
		handleError(e)
	}
}

async function refreshAll() {
	await Promise.all([refreshFolders(), refreshFiles()])
}

// ---------- selecting and reading a schematic ----------

let selectTicket = 0

async function selectFile(file: SchematicFile) {
	selected.value = file
	parsed.value = null
	parseFailed.value = false
	if (!isReadableSchematic(file.name)) return

	const ticket = ++selectTicket
	parsing.value = true
	try {
		const bytes = await schematics_read(instanceDir.value, file.path)
		const result = await parseSchematicFile(bytes, file.name)
		if (ticket === selectTicket) parsed.value = result
	} catch {
		if (ticket === selectTicket) parseFailed.value = true
	} finally {
		if (ticket === selectTicket) parsing.value = false
	}
}

// ---------- adding, creating, deleting ----------

async function importPaths(paths: string[]) {
	if (!instanceDir.value || paths.length === 0) return
	try {
		await schematics_import(instanceDir.value, currentFolder.value, paths)
		await refreshAll()
	} catch (e) {
		handleError(e)
	}
}

async function chooseFiles() {
	const chosen = await open({
		multiple: true,
		filters: [{ name: 'Schematics', extensions: ['litematic', 'schematic', 'schem', 'nbt'] }],
	})
	if (!chosen) return
	await importPaths(Array.isArray(chosen) ? chosen : [chosen])
}

async function createFolder() {
	const name = newFolderName.value.trim().replace(/[\\/]/g, '')
	if (!name) return
	const path = currentFolder.value ? `${currentFolder.value}/${name}` : name
	try {
		await schematics_create_folder(instanceDir.value, path)
		creatingFolder.value = false
		newFolderName.value = ''
		if (currentFolder.value) expanded.value = new Set(expanded.value).add(currentFolder.value)
		await refreshFolders()
	} catch (e) {
		handleError(e)
	}
}

function askDelete(path: string, name: string) {
	pendingDelete.value = { path, name }
	deleteModal.value?.show()
}

async function confirmDelete() {
	const target = pendingDelete.value
	if (!target) return
	try {
		await schematics_delete(instanceDir.value, target.path)
		if (currentFolder.value === target.path || currentFolder.value.startsWith(`${target.path}/`)) {
			currentFolder.value = ''
		}
		pendingDelete.value = null
		await refreshAll()
	} catch (e) {
		handleError(e)
	}
}

async function openInFolder() {
	const { showInstanceInFolder } = await import('@/helpers/utils')
	await showInstanceInFolder(instanceId.value)
}

// ---------- drag and drop from the file explorer ----------

let unlistenDrop: (() => void) | null = null
let unmounted = false

onMounted(async () => {
	try {
		instance.value = await getInstance(instanceId.value)
		instanceDir.value = await get_full_path(instanceId.value)
		await refreshAll()
	} catch (e) {
		handleError(e)
	}

	try {
		const unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
			const payload = event.payload
			if (payload.type === 'enter' || payload.type === 'over') {
				dragging.value = true
			} else if (payload.type === 'leave') {
				dragging.value = false
			} else if (payload.type === 'drop') {
				dragging.value = false
				await importPaths(payload.paths)
			}
		})
		if (unmounted) unlisten()
		else unlistenDrop = unlisten
	} catch (e) {
		handleError(e)
	}
})

onBeforeUnmount(() => {
	unmounted = true
	unlistenDrop?.()
})

// ---------- display helpers ----------

function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
	return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function formatTime(ms: number): string {
	if (!ms) return '-'
	return new Intl.DateTimeFormat(locale.value, { dateStyle: 'medium', timeStyle: 'medium' }).format(
		new Date(ms),
	)
}

const infoRows = computed<{ label: string; value: string }[]>(() => {
	const info = parsed.value?.info
	if (!info) return []
	const size = info.enclosingSize
	const minecraft = info.minecraftVersion
		? `${info.minecraftVersion} [${formatMessage(messages.schema, { version: info.dataVersion })}]`
		: formatMessage(messages.schema, { version: info.dataVersion })
	const rows = [
		{ label: formatMessage(messages.infoName), value: info.name || selected.value?.name || '' },
		{
			label: formatMessage(messages.infoFormat),
			value: formatMessage(
				info.format === 'sponge'
					? messages.formatSponge
					: info.format === 'structure'
						? messages.formatStructure
						: messages.formatLitematic,
			),
		},
		{ label: formatMessage(messages.infoAuthor), value: info.author || '-' },
		{ label: formatMessage(messages.infoCreated), value: formatTime(info.timeCreated) },
		{ label: formatMessage(messages.infoModified), value: formatTime(info.timeModified) },
		{ label: formatMessage(messages.infoRegions), value: String(info.regionCount) },
		{
			label: formatMessage(messages.infoVolume),
			value: formatMessage(messages.blocksUnit, { count: info.totalVolume }),
		},
		{
			label: formatMessage(messages.infoBlocks),
			value: String(info.totalBlocks),
		},
		{
			label: formatMessage(messages.infoSize),
			value: `${size.x} x ${size.y} x ${size.z}`,
		},
		...(info.format === 'litematic'
			? [{ label: formatMessage(messages.infoVersion), value: String(info.version) }]
			: info.format === 'sponge'
				? [{ label: formatMessage(messages.infoSchemVersion), value: String(info.version) }]
				: []),
		{ label: formatMessage(messages.infoMinecraft), value: minecraft },
	]
	if (info.description) {
		rows.push({ label: formatMessage(messages.infoDescription), value: info.description })
	}
	return rows
})
</script>

<template>
	<ConfirmModal
		ref="deleteModal"
		:title="formatMessage(messages.deleteTitle)"
		:description="formatMessage(messages.deleteDescription, { name: pendingDelete?.name ?? '' })"
		:proceed-label="formatMessage(messages.deleteFile)"
		@proceed="confirmDelete"
	/>

	<MaterialListModal
		ref="materialModal"
		:schematic="parsed"
		:game-version="instance?.game_version ?? ''"
		:storage-key="selected ? `${instanceId}|${selected.path}|${selected.modified}|${selected.size}` : ''"
		:title="parsed?.info.name || selected?.name"
	/>

	<div class="relative box-border flex h-full flex-col gap-4 overflow-hidden p-6">
		<!-- header -->
		<div class="flex items-center justify-between gap-3">
			<div class="flex min-w-0 items-center gap-3">
				<Button
					type="transparent"
					class="!px-0"
					:aria-label="formatMessage(messages.back)"
					@click="router.push(`/instance/${encodeURIComponent(instanceId)}`)"
				>
					<ArrowLeftIcon />
				</Button>
				<h1 class="m-0 truncate text-2xl font-extrabold text-contrast">
					{{ formatMessage(messages.title) }}
					<span v-if="instance" class="font-medium text-secondary"> · {{ instance.name }}</span>
				</h1>
			</div>
			<label class="flex shrink-0 cursor-pointer items-center gap-2 text-primary">
				{{ formatMessage(messages.preview) }}
				<Toggle id="schematics-preview-toggle" v-model="showPreview" />
			</label>
		</div>

		<div class="flex min-h-0 flex-1 gap-4">
			<!-- folders -->
			<div
				class="flex w-60 shrink-0 flex-col gap-2 overflow-hidden rounded-3xl border border-solid border-surface-4 bg-surface-2 p-3.5 shadow-sm"
			>
				<div class="flex items-center justify-between">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.folders) }}</span>
					<Button type="transparent" class="!px-1" @click="creatingFolder = !creatingFolder">
						<PlusIcon />
					</Button>
				</div>
				<div v-if="creatingFolder" class="flex flex-col gap-2">
					<input
						v-model="newFolderName"
						type="text"
						maxlength="48"
						:placeholder="formatMessage(messages.folderName)"
						class="rounded-lg border border-solid border-surface-5 bg-bg-input px-2 py-1 text-primary"
						@keydown.enter="createFolder"
					/>
					<Button type="outlined" @click="createFolder">{{ formatMessage(messages.create) }}</Button>
				</div>
				<div class="min-h-0 flex-1 overflow-y-auto">
					<button
						class="flex w-full cursor-pointer items-center gap-2 rounded-lg border-0 px-2 py-1.5 text-left text-primary hover:bg-surface-4"
						:class="currentFolder === '' ? 'bg-surface-4 font-semibold' : 'bg-transparent'"
						@click="selectFolder('')"
					>
						<FolderOpenIcon class="size-4 shrink-0" />
						<span class="truncate">{{ formatMessage(messages.root) }}</span>
					</button>
					<div v-for="row in folderRows" :key="row.path" class="group flex items-center">
						<button
							class="flex min-w-0 flex-1 cursor-pointer items-center gap-1 rounded-lg border-0 py-1.5 pr-2 text-left text-primary hover:bg-surface-4"
							:class="currentFolder === row.path ? 'bg-surface-4 font-semibold' : 'bg-transparent'"
							:style="{ paddingLeft: `${row.depth * 14 + 8}px` }"
							@click="selectFolder(row.path)"
						>
							<span
								class="flex size-4 shrink-0 items-center justify-center"
								@click.stop="row.hasChildren && toggleExpanded(row.path)"
							>
								<template v-if="row.hasChildren">
									<ChevronDownIcon v-if="expanded.has(row.path)" class="size-4" />
									<ChevronRightIcon v-else class="size-4" />
								</template>
							</span>
							<span class="truncate">{{ row.name }}</span>
						</button>
						<button
							class="hidden size-6 shrink-0 cursor-pointer items-center justify-center rounded border-0 bg-transparent text-secondary hover:text-red group-hover:flex"
							:aria-label="formatMessage(messages.deleteFolder)"
							@click="askDelete(row.path, row.name)"
						>
							<TrashIcon class="size-4" />
						</button>
					</div>
				</div>
				<Button type="outlined" @click="openInFolder">
					<FolderOpenIcon />
					{{ formatMessage(messages.openFolder) }}
				</Button>
			</div>

			<!-- schematic list -->
			<div
				class="flex min-w-0 flex-1 flex-col gap-3 overflow-hidden rounded-2xl border border-solid border-surface-4 bg-surface-2 p-3"
			>
				<div class="flex items-center justify-between gap-2">
					<span class="truncate font-semibold text-contrast">
						{{ currentFolder || formatMessage(messages.root) }}
					</span>
					<Button type="outlined" @click="chooseFiles">
						<UploadIcon />
						{{ formatMessage(messages.addFiles) }}
					</Button>
				</div>
				<div class="min-h-0 flex-1 overflow-y-auto">
					<p v-if="files.length === 0" class="m-0 p-4 text-center text-secondary">
						{{ formatMessage(messages.empty) }}
					</p>
					<div
						v-for="file in files"
						:key="file.path"
						class="group flex cursor-pointer items-center justify-between gap-3 rounded-2xl px-3.5 py-2.5 transition-colors hover:bg-surface-4"
						:class="selected?.path === file.path ? 'bg-surface-4' : ''"
						@click="selectFile(file)"
					>
						<div class="min-w-0">
							<div class="truncate font-semibold text-contrast">{{ file.name }}</div>
							<div class="text-sm text-secondary">
								{{ formatSize(file.size) }} · {{ formatTime(file.modified) }}
							</div>
						</div>
						<button
							class="hidden size-8 shrink-0 cursor-pointer items-center justify-center rounded-lg border-0 bg-transparent text-secondary hover:text-red group-hover:flex"
							:aria-label="formatMessage(messages.deleteFile)"
							@click.stop="askDelete(file.path, file.name)"
						>
							<TrashIcon class="size-5" />
						</button>
					</div>
				</div>
			</div>

			<!-- details and preview -->
			<div
				class="flex shrink-0 flex-col gap-3 overflow-hidden"
				:class="showPreview ? 'w-[26rem]' : 'w-80'"
			>
				<div v-if="showPreview" class="h-72 shrink-0">
					<SchematicPreview3D
						:schematic="parsed"
						:game-version="instance?.game_version ?? ''"
						:instance-id="instanceId"
						:cache-key="selected ? `${instanceId}|${selected.path}|${selected.modified}|${selected.size}` : undefined"
					/>
				</div>
				<div
					class="min-h-0 flex-1 overflow-y-auto rounded-2xl border border-solid border-surface-4 bg-surface-2 p-4"
				>
					<p v-if="!selected" class="m-0 text-secondary">{{ formatMessage(messages.selectOne) }}</p>
					<p v-else-if="parsing" class="m-0 text-secondary">
						{{ formatMessage(messages.loadingDetails) }}
					</p>
					<p v-else-if="parseFailed" class="m-0 text-secondary">
						{{ formatMessage(messages.cannotRead) }}
					</p>
					<template v-else-if="parsed">
						<div v-for="row in infoRows" :key="row.label" class="mb-2">
							<div class="text-sm text-secondary">{{ row.label }}</div>
							<div class="break-words font-semibold text-contrast">{{ row.value }}</div>
						</div>
						<Button class="mt-3 w-full" type="outlined" @click="materialModal?.show()">
							<ListIcon />
							{{ formatMessage(messages.materialList) }}
						</Button>
					</template>
					<template v-else>
						<div class="mb-2">
							<div class="text-sm text-secondary">{{ formatMessage(messages.infoName) }}</div>
							<div class="break-words font-semibold text-contrast">{{ selected.name }}</div>
						</div>
						<div class="mb-2">
							<div class="text-sm text-secondary">{{ formatMessage(messages.infoFile) }}</div>
							<div class="font-semibold text-contrast">{{ formatSize(selected.size) }}</div>
						</div>
						<p class="m-0 text-secondary">{{ formatMessage(messages.noDetails) }}</p>
					</template>
				</div>
			</div>
		</div>

		<div
			v-if="dragging"
			class="pointer-events-none absolute inset-3 z-10 flex items-center justify-center rounded-2xl border-2 border-dashed border-brand bg-black/50 text-xl font-semibold text-contrast"
		>
			{{ formatMessage(messages.drop) }}
		</div>
	</div>
</template>
