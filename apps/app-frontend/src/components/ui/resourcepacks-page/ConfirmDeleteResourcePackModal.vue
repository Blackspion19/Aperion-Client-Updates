<script setup lang="ts">
import { ConfirmModal, defineMessage, useVIntl } from '@aperion/ui'
import { ref } from 'vue'

import type { ResourcePackMetadata } from '@/helpers/resourcepack'

const { formatMessage } = useVIntl()

const emit = defineEmits<{
	confirm: [pack: ResourcePackMetadata]
}>()

const modal = ref<InstanceType<typeof ConfirmModal>>()
const target = ref<ResourcePackMetadata>()

function show(pack: ResourcePackMetadata) {
	target.value = pack
	modal.value?.show()
}

function proceed() {
	if (target.value) {
		emit('confirm', target.value)
	}
}

defineExpose({ show })

const titleMessage = defineMessage({
	id: 'app.resourcepacks.delete.title',
	defaultMessage: 'Delete resource pack?',
})
const descriptionMessage = defineMessage({
	id: 'app.resourcepacks.delete.description',
	defaultMessage:
		'This will permanently delete "{name}" and remove it from any instance it is deployed to. This cannot be undone.',
})
const proceedMessage = defineMessage({
	id: 'app.resourcepacks.delete.proceed',
	defaultMessage: 'Delete',
})
</script>
<template>
	<ConfirmModal
		ref="modal"
		:title="formatMessage(titleMessage)"
		:description="target ? formatMessage(descriptionMessage, { name: target.name }) : ''"
		:proceed-label="formatMessage(proceedMessage)"
		:markdown="false"
		danger
		@proceed="proceed"
	/>
</template>
