<script setup lang="ts">
import { Combobox, defineMessages, useVIntl } from '@aperion/ui'
import { computed, watch } from 'vue'

const props = defineProps<{
	/** All selectable versions, newest first */
	versions: string[]
}>()

const { formatMessage } = useVIntl()

// The oldest version the pack works with
const from = defineModel<string>('from', { required: true })
// The newest version: "" = only `from`, "*" = no upper limit, otherwise a version
const to = defineModel<string>('to', { default: '' })

const messages = defineMessages({
	from: { id: 'app.resourcepacks.range.from', defaultMessage: 'From version' },
	to: { id: 'app.resourcepacks.range.to', defaultMessage: 'To version' },
	only: { id: 'app.resourcepacks.range.only', defaultMessage: 'Only this version' },
	newer: { id: 'app.resourcepacks.range.newer', defaultMessage: 'This version and newer' },
	hint: {
		id: 'app.resourcepacks.range.hint',
		defaultMessage:
			'The pack is marked as working from the first to the last version. The textures themselves are not converted.',
	},
})

const SAME = '__same__'

const fromOptions = computed(() => props.versions.map((version) => ({ value: version, label: version })))

// Only versions newer than "from" can be the upper end
const newerVersions = computed(() => {
	const index = props.versions.indexOf(from.value)
	return index > 0 ? props.versions.slice(0, index) : []
})

const toOptions = computed(() => [
	{ value: SAME, label: formatMessage(messages.only) },
	{ value: '*', label: formatMessage(messages.newer) },
	...newerVersions.value.map((version) => ({ value: version, label: version })),
])

const toValue = computed({
	get: () => to.value || SAME,
	set: (value: string) => {
		to.value = value === SAME ? '' : value
	},
})

const toDisplay = computed(() =>
	to.value === '' ? formatMessage(messages.only) : to.value === '*' ? formatMessage(messages.newer) : to.value,
)

// A new "from" version that is newer than the chosen upper end resets the upper end.
watch(from, () => {
	if (to.value && to.value !== '*' && !newerVersions.value.includes(to.value)) to.value = ''
})
</script>
<template>
	<div class="flex flex-col gap-2">
		<div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
			<div class="flex min-w-0 flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{ formatMessage(messages.from) }}</span>
				<Combobox
					v-model="from"
					:options="fromOptions"
					searchable
					sync-with-selection
					:max-height="280"
					:placeholder="from || formatMessage(messages.from)"
					:search-placeholder="formatMessage(messages.from)"
					:display-value="from || formatMessage(messages.from)"
					:aria-label="formatMessage(messages.from)"
				/>
			</div>
			<div class="flex min-w-0 flex-col gap-1">
				<span class="text-sm font-semibold text-contrast">{{ formatMessage(messages.to) }}</span>
				<Combobox
					v-model="toValue"
					:options="toOptions"
					searchable
					sync-with-selection
					:max-height="280"
					:placeholder="toDisplay"
					:search-placeholder="formatMessage(messages.to)"
					:display-value="toDisplay"
					:aria-label="formatMessage(messages.to)"
				/>
			</div>
		</div>
		<span class="text-sm text-secondary">{{ formatMessage(messages.hint) }}</span>
	</div>
</template>
