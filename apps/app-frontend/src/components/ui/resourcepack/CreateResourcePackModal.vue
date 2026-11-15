<script setup lang="ts">
import { PlusIcon, XIcon } from '@aperion/assets'
import { Button, Combobox, commonMessages, defineMessage, defineMessages, Input, NewModal, useVIntl } from '@aperion/ui'
import { computed, ref } from 'vue'

import type { ResourcePackMetadata } from '@/helpers/resourcepack'
import { create_resourcepack, list_available_game_versions } from '@/helpers/resourcepack'
import VersionRangeSelect from './VersionRangeSelect.vue'

const { formatMessage } = useVIntl()

const emit = defineEmits<{
	created: [pack: ResourcePackMetadata]
}>()

const modal = ref<InstanceType<typeof NewModal>>()
const name = ref('')
const description = ref('')
const gameVersion = ref('')
const maxVersion = ref('')
const availableVersions = ref<string[]>([])
const versionOptions = computed(() =>
	availableVersions.value.map((version) => ({ value: version, label: version })),
)
const creating = ref(false)
const error = ref<string | null>(null)

const canCreate = computed(
	() => name.value.trim().length > 0 && gameVersion.value.trim().length > 0 && !creating.value,
)

async function show() {
	name.value = ''
	description.value = ''
	error.value = null

	try {
		availableVersions.value = await list_available_game_versions()
		gameVersion.value = availableVersions.value[0] ?? ''
	} catch {
		availableVersions.value = []
	}

	modal.value?.show()
}

async function create() {
	if (!canCreate.value) return
	creating.value = true
	error.value = null
	try {
		const pack = await create_resourcepack(
			name.value.trim(),
			description.value.trim(),
			gameVersion.value.trim(),
			maxVersion.value || null,
		)
		emit('created', pack)
		hide()
	} catch (e) {
		error.value = e instanceof Error ? e.message : String(e)
	} finally {
		creating.value = false
	}
}

function hide() {
	modal.value?.hide()
}

defineExpose({ show })

const titleMessage = defineMessage({
	id: 'app.resourcepacks.create.title',
	defaultMessage: 'Create resource pack',
})
const nameLabel = defineMessage({
	id: 'app.resourcepacks.create.name',
	defaultMessage: 'Name',
})
const descriptionLabel = defineMessage({
	id: 'app.resourcepacks.create.description',
	defaultMessage: 'Description',
})
const versionLabel = defineMessage({
	id: 'app.resourcepacks.create.version',
	defaultMessage: 'Minecraft version',
})
const versionHint = defineMessage({
	id: 'app.resourcepacks.create.version-hint',
	defaultMessage:
		'Only versions you have an instance for are shown, since the vanilla assets need to be downloaded first.',
})
const noVersionsMessage = defineMessage({
	id: 'app.resourcepacks.create.no-versions',
	defaultMessage: 'Create an instance first so there is a Minecraft version to base this on.',
})
const messages = defineMessages({
	createButton: {
		id: 'app.resourcepacks.create.submit',
		defaultMessage: 'Create',
	},
})
</script>
<template>
	<NewModal ref="modal" :header="formatMessage(titleMessage)" width="480px" max-width="480px">
		<div class="flex flex-col gap-3">
			<label class="flex flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{ formatMessage(nameLabel) }}</span>
				<Input v-model="name" autocomplete="off" wrapper-class="w-full" maxlength="64" />
			</label>
			<label class="flex flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{
					formatMessage(descriptionLabel)
				}}</span>
				<Input v-model="description" autocomplete="off" wrapper-class="w-full" maxlength="256" />
			</label>
			<VersionRangeSelect
				v-model:from="gameVersion"
				v-model:to="maxVersion"
				:versions="availableVersions"
			/>
			<p v-if="availableVersions.length === 0" class="m-0 text-sm text-orange">
				{{ formatMessage(noVersionsMessage) }}
			</p>
			<p v-if="error" class="m-0 text-sm text-red">{{ error }}</p>
		</div>
		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" :disabled="!canCreate" @click="create">
					<PlusIcon />
					{{ formatMessage(messages.createButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
