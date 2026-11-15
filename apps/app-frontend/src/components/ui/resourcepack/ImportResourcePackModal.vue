<script setup lang="ts">
import { PlusIcon, XIcon } from '@aperion/assets'
import { Button, Combobox, commonMessages, defineMessage, Input, NewModal, useVIntl } from '@aperion/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import {
	import_resourcepack,
	list_available_game_versions,
	type ResourcePackMetadata,
} from '@/helpers/resourcepack'
import VersionRangeSelect from './VersionRangeSelect.vue'

const { formatMessage } = useVIntl()

const emit = defineEmits<{
	imported: [pack: ResourcePackMetadata]
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const zipPath = ref<string | null>(null)
const name = ref('')
const gameVersion = ref('')
const maxVersion = ref('')
const availableVersions = ref<string[]>([])
const versionOptions = computed(() =>
	availableVersions.value.map((version) => ({ value: version, label: version })),
)
const importing = ref(false)
const error = ref<string | null>(null)

const canImport = computed(
	() => !!zipPath.value && gameVersion.value.trim().length > 0 && !importing.value,
)

function fileNameOf(path: string): string {
	return path.split(/[\\/]/).pop() ?? path
}

async function chooseFile() {
	const selected = await open({
		multiple: false,
		filters: [{ name: 'Resource pack', extensions: ['zip'] }],
	})
	if (typeof selected !== 'string') return
	zipPath.value = selected
	if (!name.value.trim()) {
		name.value = fileNameOf(selected)
			.replace(/\.zip$/i, '')
			.slice(0, 64)
	}
}

async function show() {
	zipPath.value = null
	name.value = ''
	error.value = null
	try {
		availableVersions.value = await list_available_game_versions()
		gameVersion.value = availableVersions.value[0] ?? ''
	} catch {
		availableVersions.value = []
	}
	modal.value?.show()
}

async function doImport() {
	if (!canImport.value || !zipPath.value) return
	importing.value = true
	error.value = null
	try {
		const pack = await import_resourcepack(
			zipPath.value,
			name.value.trim() || null,
			gameVersion.value.trim(),
			maxVersion.value || null,
		)
		emit('imported', pack)
		modal.value?.hide()
	} catch (e) {
		error.value = e instanceof Error ? e.message : String(e)
	} finally {
		importing.value = false
	}
}

defineExpose({ show })

const titleMessage = defineMessage({
	id: 'app.resourcepacks.import.title',
	defaultMessage: 'Import resource pack',
})
const fileLabel = defineMessage({
	id: 'app.resourcepacks.import.file',
	defaultMessage: 'Resource pack file (.zip)',
})
const chooseButton = defineMessage({
	id: 'app.resourcepacks.import.choose',
	defaultMessage: 'Choose file',
})
const noFile = defineMessage({
	id: 'app.resourcepacks.import.no-file',
	defaultMessage: 'No file selected',
})
const nameLabel = defineMessage({
	id: 'app.resourcepacks.create.name',
	defaultMessage: 'Name',
})
const versionLabel = defineMessage({
	id: 'app.resourcepacks.create.version',
	defaultMessage: 'Minecraft version',
})
const importButton = defineMessage({
	id: 'app.resourcepacks.import.submit',
	defaultMessage: 'Import',
})
const hint = defineMessage({
	id: 'app.resourcepacks.import.hint',
	defaultMessage:
		'Only textures, sounds and other files in the assets folder are imported. The pack.mcmeta is created again for the selected version.',
})
</script>
<template>
	<NewModal ref="modal" :header="formatMessage(titleMessage)" width="480px" max-width="480px">
		<div class="flex flex-col gap-3">
			<div class="flex flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{ formatMessage(fileLabel) }}</span>
				<div class="flex items-center gap-2">
					<Button type="outlined" @click="chooseFile">{{ formatMessage(chooseButton) }}</Button>
					<span class="min-w-0 truncate text-sm text-secondary">
						{{ zipPath ? fileNameOf(zipPath) : formatMessage(noFile) }}
					</span>
				</div>
			</div>
			<label class="flex flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{ formatMessage(nameLabel) }}</span>
				<Input v-model="name" autocomplete="off" wrapper-class="w-full" maxlength="64" />
			</label>
			<VersionRangeSelect
				v-model:from="gameVersion"
				v-model:to="maxVersion"
				:versions="availableVersions"
			/>
			<span class="text-sm text-secondary">{{ formatMessage(hint) }}</span>
			<p v-if="error" class="m-0 text-sm text-red">{{ error }}</p>
		</div>
		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" :disabled="!canImport" @click="doImport">
					<PlusIcon />
					{{ formatMessage(importButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
