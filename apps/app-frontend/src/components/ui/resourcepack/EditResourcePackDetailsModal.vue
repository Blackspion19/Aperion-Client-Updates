<script setup lang="ts">
import { SaveIcon, XIcon } from '@aperion/assets'
import { Button, Combobox, commonMessages, defineMessage, Input, injectNotificationManager, NewModal, useVIntl } from '@aperion/ui'
import { computed, ref, watch } from 'vue'

import {
	edit_resourcepack,
	list_available_game_versions,
	retarget_resourcepack,
	type ResourcePackMetadata,
} from '@/helpers/resourcepack'

const props = defineProps<{
	pack: ResourcePackMetadata
}>()

const emit = defineEmits<{
	saved: [pack: ResourcePackMetadata]
}>()
import VersionRangeSelect from './VersionRangeSelect.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const modal = ref<InstanceType<typeof NewModal>>()
const name = ref(props.pack.name)
const description = ref(props.pack.description)
const gameVersion = ref(props.pack.game_version)
const maxVersion = ref('')
const availableVersions = ref<string[]>([])
const versionOptions = computed(() =>
	availableVersions.value.map((version) => ({ value: version, label: version })),
)
const saving = ref(false)

watch(
	() => props.pack,
	(pack) => {
		name.value = pack.name
		description.value = pack.description
		gameVersion.value = pack.game_version
		maxVersion.value = pack.max_game_version ?? ''
	},
)

async function show() {
	name.value = props.pack.name
	description.value = props.pack.description
	gameVersion.value = props.pack.game_version
	maxVersion.value = props.pack.max_game_version ?? ''
	try {
		const versions = await list_available_game_versions()
		// The current version stays selectable even without a matching instance.
		availableVersions.value = versions.includes(props.pack.game_version)
			? versions
			: [props.pack.game_version, ...versions]
	} catch {
		availableVersions.value = [props.pack.game_version]
	}
	modal.value?.show()
}

function hide() {
	modal.value?.hide()
}

async function save() {
	if (!name.value.trim()) return
	saving.value = true
	try {
		let updated = await edit_resourcepack(
			props.pack.id,
			name.value.trim(),
			description.value.trim(),
		)
		const newMax = maxVersion.value || null
		if (
			gameVersion.value &&
			(gameVersion.value !== props.pack.game_version || newMax !== (props.pack.max_game_version ?? null))
		) {
			updated = await retarget_resourcepack(props.pack.id, gameVersion.value, newMax)
		}
		emit('saved', updated)
		hide()
	} catch (e) {
		handleError(e)
	} finally {
		saving.value = false
	}
}

defineExpose({ show })

const titleMessage = defineMessage({
	id: 'app.resourcepacks.edit-details.title',
	defaultMessage: 'Edit resource pack',
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
	id: 'app.resourcepacks.edit-details.version-hint',
	defaultMessage:
		'Changing the version updates the pack format. The textures and sounds themselves are not converted.',
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
		</div>
		<template #actions>
			<div class="flex gap-2 justify-end">
				<Button type="outlined" @click="hide()">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button
					type="colored"
					color="brand"
					:disabled="!name.trim() || saving"
					@click="save"
				>
					<SaveIcon />
					{{ formatMessage(commonMessages.saveChangesButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
