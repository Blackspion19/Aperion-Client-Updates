<script setup lang="ts">
import { CheckIcon, SearchIcon, XIcon } from '@aperion/assets'
import { Button, defineMessages, NewModal, Toggle, useVIntl } from '@aperion/ui'
import { computed, ref } from 'vue'

import type { ParsedSchematic } from '@/helpers/litematic'
import { countMaterials, type MaterialEntry, stackText } from '@/helpers/schematic-materials'

import MaterialIcon from './MaterialIcon.vue'

const props = defineProps<{
	schematic: ParsedSchematic | null
	gameVersion: string
	/** Identifies the schematic, so the ticked items are remembered per schematic */
	storageKey: string
	title?: string
}>()

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
	title: { id: 'app.schematics.materials.title', defaultMessage: 'Material list' },
	progress: {
		id: 'app.schematics.materials.progress',
		defaultMessage: '{done} of {total} materials done',
	},
	totalItems: {
		id: 'app.schematics.materials.total-items',
		defaultMessage: '{count} items in total',
	},
	search: { id: 'app.schematics.materials.search', defaultMessage: 'Search materials' },
	sortCount: { id: 'app.schematics.materials.sort-count', defaultMessage: 'Most needed' },
	sortName: { id: 'app.schematics.materials.sort-name', defaultMessage: 'Name' },
	sortOpen: { id: 'app.schematics.materials.sort-open', defaultMessage: 'Not done first' },
	hideDone: { id: 'app.schematics.materials.hide-done', defaultMessage: 'Hide completed' },
	checkAll: { id: 'app.schematics.materials.check-all', defaultMessage: 'Check all' },
	reset: { id: 'app.schematics.materials.reset', defaultMessage: 'Reset' },
	confirmReset: { id: 'app.schematics.materials.confirm-reset', defaultMessage: 'Really reset?' },
	copy: { id: 'app.schematics.materials.copy', defaultMessage: 'Copy list' },
	copied: { id: 'app.schematics.materials.copied', defaultMessage: 'Copied' },
	calculating: { id: 'app.schematics.materials.calculating', defaultMessage: 'Counting blocks...' },
	empty: { id: 'app.schematics.materials.empty', defaultMessage: 'Nothing to show.' },
	allDone: { id: 'app.schematics.materials.all-done', defaultMessage: 'Everything is done!' },
	note: {
		id: 'app.schematics.materials.note',
		defaultMessage: 'Water and lava are not counted. Slabs, doors and beds are counted as items.',
	},
	close: { id: 'app.schematics.materials.close', defaultMessage: 'Close' },
})

type Sort = 'count' | 'name' | 'open'

const modal = ref<InstanceType<typeof NewModal>>()
const entries = ref<MaterialEntry[]>([])
const loading = ref(false)
const checked = ref(new Set<string>())
const search = ref('')
const sort = ref<Sort>('count')
const hideDone = ref(false)
const confirmingReset = ref(false)
const copiedNotice = ref(false)

// Counts stay in memory per schematic, so reopening the list is instant.
const cache = new Map<string, MaterialEntry[]>()
let ticket = 0
let resetTimer: ReturnType<typeof setTimeout> | undefined

const storageName = computed(() => `aperion-materials:${props.storageKey}`)

function loadChecked() {
	try {
		const raw = window.localStorage.getItem(storageName.value)
		checked.value = new Set(raw ? (JSON.parse(raw) as string[]) : [])
	} catch {
		checked.value = new Set()
	}
}

function saveChecked() {
	try {
		window.localStorage.setItem(storageName.value, JSON.stringify([...checked.value]))
	} catch {
		// storage blocked or full
	}
}

async function show() {
	modal.value?.show()
	search.value = ''
	confirmingReset.value = false
	loadChecked()

	const cached = cache.get(props.storageKey)
	if (cached) {
		entries.value = cached
		return
	}
	if (!props.schematic) return

	const current = ++ticket
	loading.value = true
	entries.value = []
	try {
		const result = await countMaterials(props.schematic, () => current !== ticket)
		if (current === ticket) {
			entries.value = result
			cache.set(props.storageKey, result)
			if (cache.size > 8) cache.delete(cache.keys().next().value as string)
		}
	} finally {
		if (current === ticket) loading.value = false
	}
}

function toggle(id: string) {
	const next = new Set(checked.value)
	if (next.has(id)) next.delete(id)
	else next.add(id)
	checked.value = next
	saveChecked()
}

function checkAll() {
	checked.value = new Set(entries.value.map((entry) => entry.id))
	saveChecked()
}

function reset() {
	if (!confirmingReset.value) {
		// Two clicks: the first only asks, the second resets.
		confirmingReset.value = true
		clearTimeout(resetTimer)
		resetTimer = setTimeout(() => (confirmingReset.value = false), 3000)
		return
	}
	clearTimeout(resetTimer)
	confirmingReset.value = false
	checked.value = new Set()
	saveChecked()
}

const doneCount = computed(() => entries.value.filter((entry) => checked.value.has(entry.id)).length)
const percent = computed(() =>
	entries.value.length === 0 ? 0 : Math.round((doneCount.value / entries.value.length) * 100),
)
const totalItems = computed(() => entries.value.reduce((sum, entry) => sum + entry.count, 0))

const visibleEntries = computed(() => {
	const query = search.value.trim().toLowerCase()
	let list = entries.value.filter(
		(entry) =>
			(!query || entry.name.toLowerCase().includes(query) || entry.id.toLowerCase().includes(query)) &&
			(!hideDone.value || !checked.value.has(entry.id)),
	)
	if (sort.value === 'name') list = [...list].sort((a, b) => a.name.localeCompare(b.name))
	else if (sort.value === 'open') {
		list = [...list].sort(
			(a, b) =>
				Number(checked.value.has(a.id)) - Number(checked.value.has(b.id)) || b.count - a.count,
		)
	}
	return list
})

function formatCount(count: number): string {
	return new Intl.NumberFormat(locale.value).format(count)
}

async function copyList() {
	// Only what is still missing; the whole list when everything is done
	const open = entries.value.filter((entry) => !checked.value.has(entry.id))
	const list = open.length > 0 ? open : entries.value
	const text = [
		props.title ?? formatMessage(messages.title),
		...list.map((entry) => {
			const stacks = stackText(entry.count)
			return `${formatCount(entry.count)} x ${entry.name}${stacks ? ` (${stacks})` : ''}`
		}),
	].join('\n')
	try {
		await navigator.clipboard.writeText(text)
	} catch {
		// Fallback for webviews without the clipboard API
		const area = document.createElement('textarea')
		area.value = text
		document.body.appendChild(area)
		area.select()
		document.execCommand('copy')
		area.remove()
	}
	copiedNotice.value = true
	setTimeout(() => (copiedNotice.value = false), 1600)
}

defineExpose({ show })
</script>
<template>
	<NewModal
		ref="modal"
		:header="`${formatMessage(messages.title)}${title ? ` · ${title}` : ''}`"
		width="720px"
		max-width="720px"
		:on-hide="() => ticket++"
	>
		<div class="flex flex-col gap-3">
			<div class="flex flex-col gap-1.5">
				<div class="flex items-center justify-between gap-2 text-sm">
					<span class="font-semibold text-contrast">
						{{ formatMessage(messages.progress, { done: doneCount, total: entries.length }) }}
					</span>
					<span class="text-secondary">
						{{ formatMessage(messages.totalItems, { count: formatCount(totalItems) }) }}
					</span>
				</div>
				<div class="h-2.5 w-full overflow-hidden rounded-full bg-surface-4">
					<div
						class="h-full rounded-full bg-brand transition-[width] duration-300"
						:style="{ width: `${percent}%` }"
					></div>
				</div>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<div class="flex min-w-48 flex-1 items-center gap-2 rounded-xl border border-solid border-surface-5 bg-bg-input px-3">
					<SearchIcon class="size-4 shrink-0 text-secondary" />
					<input
						v-model="search"
						type="text"
						:placeholder="formatMessage(messages.search)"
						class="w-full border-0 bg-transparent py-2 text-primary outline-none"
					/>
				</div>
				<select
					v-model="sort"
					class="rounded-xl border border-solid border-surface-5 bg-bg-input px-3 py-2 text-primary"
				>
					<option value="count">{{ formatMessage(messages.sortCount) }}</option>
					<option value="name">{{ formatMessage(messages.sortName) }}</option>
					<option value="open">{{ formatMessage(messages.sortOpen) }}</option>
				</select>
				<label class="flex cursor-pointer items-center gap-2 text-sm text-primary">
					<Toggle id="materials-hide-done" v-model="hideDone" />
					{{ formatMessage(messages.hideDone) }}
				</label>
			</div>

			<div class="max-h-[50vh] min-h-48 overflow-y-auto rounded-2xl border border-solid border-surface-4 bg-surface-2 p-2">
				<p v-if="loading" class="m-0 p-6 text-center text-secondary">
					{{ formatMessage(messages.calculating) }}
				</p>
				<p v-else-if="entries.length === 0" class="m-0 p-6 text-center text-secondary">
					{{ formatMessage(messages.empty) }}
				</p>
				<p
					v-else-if="visibleEntries.length === 0"
					class="m-0 p-6 text-center text-secondary"
				>
					{{ hideDone && doneCount === entries.length ? formatMessage(messages.allDone) : formatMessage(messages.empty) }}
				</p>
				<div
					v-for="entry in visibleEntries"
					:key="entry.id"
					role="checkbox"
					tabindex="0"
					:aria-checked="checked.has(entry.id)"
					class="flex cursor-pointer select-none items-center gap-3 rounded-xl px-3 py-2 transition-colors hover:bg-surface-4"
					:class="checked.has(entry.id) ? 'opacity-50' : ''"
					@click="toggle(entry.id)"
					@keydown.enter.prevent="toggle(entry.id)"
					@keydown.space.prevent="toggle(entry.id)"
				>
					<span
						class="flex size-6 shrink-0 items-center justify-center rounded-lg border-2 border-solid transition-colors"
						:class="
							checked.has(entry.id)
								? 'border-brand bg-brand text-accent-contrast'
								: 'border-surface-5 bg-transparent'
						"
					>
						<CheckIcon v-if="checked.has(entry.id)" class="size-4" />
					</span>
					<MaterialIcon :id="entry.id" :game-version="gameVersion" />
					<span class="min-w-0 flex-1">
						<span
							class="block truncate font-semibold text-contrast"
							:class="checked.has(entry.id) ? 'line-through' : ''"
						>
							{{ entry.name }}
						</span>
						<span class="block truncate text-xs text-secondary">{{ entry.id }}</span>
					</span>
					<span class="shrink-0 text-right">
						<span class="block font-bold text-contrast">{{ formatCount(entry.count) }}</span>
						<span v-if="stackText(entry.count)" class="block text-xs text-secondary">
							{{ stackText(entry.count) }}
						</span>
					</span>
				</div>
			</div>

			<p class="m-0 text-xs text-secondary">{{ formatMessage(messages.note) }}</p>
		</div>

		<template #actions>
			<div class="flex flex-wrap items-center justify-between gap-2">
				<div class="flex flex-wrap gap-2">
					<Button type="outlined" :disabled="entries.length === 0" @click="checkAll">
						<CheckIcon />
						{{ formatMessage(messages.checkAll) }}
					</Button>
					<Button
						type="outlined"
						:color="confirmingReset ? 'red' : undefined"
						:disabled="checked.size === 0"
						@click="reset"
					>
						{{ formatMessage(confirmingReset ? messages.confirmReset : messages.reset) }}
					</Button>
					<Button type="outlined" :disabled="entries.length === 0" @click="copyList">
						{{ formatMessage(copiedNotice ? messages.copied : messages.copy) }}
					</Button>
				</div>
				<Button type="outlined" @click="modal?.hide()">
					<XIcon />
					{{ formatMessage(messages.close) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
