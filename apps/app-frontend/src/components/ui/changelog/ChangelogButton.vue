<template>
	<div class="relative">
		<Button type="outlined" native-type="button" class="!h-[34px] text-sm" @click="open">
			{{ formatMessage(messages.button) }}
		</Button>
		<span
			v-if="hasUnseen"
			class="pointer-events-none absolute -right-0.5 -top-0.5 size-2.5 rounded-full bg-brand"
			aria-hidden="true"
		/>
		<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="640px" scrollable>
			<div class="flex flex-col gap-6">
				<ChangelogEntry
					v-for="(entry, index) in entries"
					:key="entry.version"
					:entry="entry"
					:first="index === 0"
				/>
			</div>
		</NewModal>
	</div>
</template>

<script setup lang="ts">
import { Button, ChangelogEntry, defineMessages, NewModal, useVIntl } from '@aperion/ui'
import dayjs from 'dayjs'
import { computed, ref } from 'vue'

import { CHANGELOG } from '@/changelog'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	button: {
		id: 'app.changelog.button',
		defaultMessage: 'Changelog',
	},
	title: {
		id: 'app.changelog.title',
		defaultMessage: 'Changelog',
	},
})

const SEEN_KEY = 'aperion:changelog-seen'

const entries = CHANGELOG.map((item) => ({
	product: 'app' as const,
	version: item.version,
	date: dayjs(`${item.date}T12:00:00Z`),
	body: item.body,
}))

const latestVersion = entries[0]?.version

function readSeenVersion(): string | null {
	try {
		return localStorage.getItem(SEEN_KEY)
	} catch {
		return null
	}
}

const seenVersion = ref(readSeenVersion())
const hasUnseen = computed(() => !!latestVersion && seenVersion.value !== latestVersion)

const modal = ref<InstanceType<typeof NewModal>>()

function open() {
	modal.value?.show()
	if (!latestVersion) return
	seenVersion.value = latestVersion
	try {
		localStorage.setItem(SEEN_KEY, latestVersion)
	} catch {
		// Nicht kritisch: dann erscheint der Punkt beim nächsten Start erneut.
	}
}
</script>
