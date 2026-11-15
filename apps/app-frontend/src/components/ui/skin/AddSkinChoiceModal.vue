<script setup lang="ts">
import { PaintbrushIcon, UploadIcon } from '@aperion/assets'
import { defineMessages, NewModal, useVIntl } from '@aperion/ui'
import { ref } from 'vue'

const { formatMessage } = useVIntl()

const emit = defineEmits<{
	file: []
	draw: []
}>()

const modal = ref<InstanceType<typeof NewModal>>()

const messages = defineMessages({
	title: { id: 'app.skins.add-choice.title', defaultMessage: 'Add a skin' },
	fromFile: { id: 'app.skins.add-choice.from-file', defaultMessage: 'From file' },
	fromFileDescription: {
		id: 'app.skins.add-choice.from-file.description',
		defaultMessage: 'Choose a skin image (.png) from a folder on your computer.',
	},
	draw: { id: 'app.skins.add-choice.draw', defaultMessage: 'Draw' },
	drawDescription: {
		id: 'app.skins.add-choice.draw.description',
		defaultMessage: 'Paint your own skin, starting from the Steve skin.',
	},
})

function choose(kind: 'file' | 'draw') {
	modal.value?.hide()
	if (kind === 'file') emit('file')
	else emit('draw')
}

defineExpose({ show: () => modal.value?.show() })
</script>
<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" width="460px" max-width="460px">
		<div class="flex flex-col gap-3">
			<button
				class="flex cursor-pointer items-center gap-4 rounded-2xl border border-solid border-surface-5 bg-surface-3 p-4 text-left hover:bg-surface-4"
				@click="choose('file')"
			>
				<UploadIcon class="size-8 shrink-0 text-brand" />
				<span>
					<span class="block text-lg font-semibold text-contrast">
						{{ formatMessage(messages.fromFile) }}
					</span>
					<span class="block text-secondary">{{ formatMessage(messages.fromFileDescription) }}</span>
				</span>
			</button>
			<button
				class="flex cursor-pointer items-center gap-4 rounded-2xl border border-solid border-surface-5 bg-surface-3 p-4 text-left hover:bg-surface-4"
				@click="choose('draw')"
			>
				<PaintbrushIcon class="size-8 shrink-0 text-brand" />
				<span>
					<span class="block text-lg font-semibold text-contrast">
						{{ formatMessage(messages.draw) }}
					</span>
					<span class="block text-secondary">{{ formatMessage(messages.drawDescription) }}</span>
				</span>
			</button>
		</div>
	</NewModal>
</template>
