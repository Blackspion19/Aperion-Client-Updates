<script setup lang="ts">
import { Button, defineMessages, injectNotificationManager, useVIntl } from '@aperion/ui'
import { ref } from 'vue'

import { useCustomization } from '@/composables/use-customization'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const customization = useCustomization()
const { state } = customization

const messages = defineMessages({
	title: { id: 'app.customization.title', defaultMessage: 'Personalization' },
	description: {
		id: 'app.customization.description',
		defaultMessage: 'Make the launcher your own with a custom background and your own colors.',
	},
	backgroundTitle: { id: 'app.customization.background.title', defaultMessage: 'Background image' },
	backgroundDescription: {
		id: 'app.customization.background.description',
		defaultMessage:
			'The image stays on this computer. Large images are scaled down automatically.',
	},
	choose: { id: 'app.customization.background.choose', defaultMessage: 'Choose image' },
	remove: { id: 'app.customization.background.remove', defaultMessage: 'Remove image' },
	panelOpacity: {
		id: 'app.customization.panel-opacity',
		defaultMessage: 'Panel opacity: {value}%',
	},
	dim: { id: 'app.customization.dim', defaultMessage: 'Darken image: {value}%' },
	blur: { id: 'app.customization.blur', defaultMessage: 'Blur: {value}px' },
	colorsTitle: { id: 'app.customization.colors.title', defaultMessage: 'Colors' },
	accent: { id: 'app.customization.accent', defaultMessage: 'Accent color' },
	accentDescription: {
		id: 'app.customization.accent.description',
		defaultMessage: 'Buttons, highlights and selected items.',
	},
	base: { id: 'app.customization.base', defaultMessage: 'Background color' },
	baseDescription: {
		id: 'app.customization.base.description',
		defaultMessage: 'Base color of all panels. Works with the dark themes.',
	},
	useDefault: { id: 'app.customization.default', defaultMessage: 'Default' },
	resetAll: { id: 'app.customization.reset-all', defaultMessage: 'Reset everything' },
	presets: { id: 'app.customization.presets', defaultMessage: 'Quick picks' },
	themes: { id: 'app.customization.themes', defaultMessage: 'Themes' },
	themesDescription: {
		id: 'app.customization.themes.description',
		defaultMessage:
			'One click sets accent and background color. You can adjust both afterwards.',
	},
})

const fileInput = ref<HTMLInputElement>()
const busy = ref(false)

const ACCENT_PRESETS = ['#3b82f6', '#22c55e', '#a855f7', '#ec4899', '#f97316', '#ef4444', '#14b8a6', '#eab308']
const BASE_PRESETS = ['#070a12', '#0b0b0f', '#0f0a14', '#0a1210', '#140a0a', '#121212', '#0a1020', '#1a1a2e']

// Ready-made themes. They only set the accent and background color, which you can still change
// afterwards. "Default" (both null) is the launcher's normal look and is what everyone starts with.
type ColorTheme = { id: string; name: string; accent: string | null; base: string | null }
const THEMES: ColorTheme[] = [
	{ id: 'default', name: '', accent: null, base: null },
	{ id: 'midnight', name: 'Midnight', accent: '#3b82f6', base: '#0a1020' },
	{ id: 'emerald', name: 'Emerald', accent: '#22c55e', base: '#0a1210' },
	{ id: 'violet', name: 'Violet', accent: '#a855f7', base: '#0f0a14' },
	{ id: 'sunset', name: 'Sunset', accent: '#f97316', base: '#140a0a' },
	{ id: 'rose', name: 'Rose', accent: '#ec4899', base: '#0b0b0f' },
	{ id: 'ocean', name: 'Ocean', accent: '#14b8a6', base: '#0a1020' },
	{ id: 'mono', name: 'Mono', accent: '#d4d4d8', base: '#121212' },
]

function isThemeActive(theme: ColorTheme): boolean {
	return state.accent === theme.accent && state.base === theme.base
}

function applyTheme(theme: ColorTheme) {
	state.accent = theme.accent
	state.base = theme.base
	customization.update()
}

async function onFile(event: Event) {
	const input = event.target as HTMLInputElement
	const file = input.files?.[0]
	input.value = ''
	if (!file) return
	busy.value = true
	try {
		await customization.setBackground(file)
	} catch (e) {
		handleError(e)
	} finally {
		busy.value = false
	}
}

async function removeBackground() {
	busy.value = true
	try {
		await customization.removeBackground()
	} catch (e) {
		handleError(e)
	} finally {
		busy.value = false
	}
}

function setAccent(value: string | null) {
	state.accent = value
	customization.update()
}

function setBase(value: string | null) {
	state.base = value
	customization.update()
}
</script>
<template>
	<section class="mt-8 flex flex-col gap-6 border-0 border-t border-solid border-surface-4 pt-6">
		<div>
			<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
			<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.description) }}</p>
		</div>

		<div class="flex flex-col gap-3">
			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.backgroundTitle) }}
				</h3>
				<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.backgroundDescription) }}</p>
			</div>
			<input ref="fileInput" type="file" accept="image/*" class="hidden" @change="onFile" />
			<div class="flex flex-wrap gap-2">
				<Button type="colored" color="brand" :disabled="busy" @click="fileInput?.click()">
					{{ formatMessage(messages.choose) }}
				</Button>
				<Button v-if="state.hasBackground" type="outlined" :disabled="busy" @click="removeBackground">
					{{ formatMessage(messages.remove) }}
				</Button>
			</div>

			<div v-if="state.hasBackground" class="flex flex-col gap-3 text-secondary">
				<label class="flex flex-col gap-1">
					{{ formatMessage(messages.panelOpacity, { value: state.panelOpacity }) }}
					<input
						v-model.number="state.panelOpacity"
						type="range"
						min="0"
						max="100"
						class="w-72 !rounded-xl"
						@input="customization.update()"
					/>
				</label>
				<label class="flex flex-col gap-1">
					{{ formatMessage(messages.dim, { value: state.dim }) }}
					<input
						v-model.number="state.dim"
						type="range"
						min="0"
						max="90"
						class="w-72 !rounded-xl"
						@input="customization.update()"
					/>
				</label>
				<label class="flex flex-col gap-1">
					{{ formatMessage(messages.blur, { value: state.blur }) }}
					<input
						v-model.number="state.blur"
						type="range"
						min="0"
						max="24"
						class="w-72 !rounded-xl"
						@input="customization.update()"
					/>
				</label>
			</div>
		</div>

		<div class="flex flex-col gap-4">
			<h3 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.colorsTitle) }}</h3>

			<div class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.themes) }}</span>
				<span class="text-secondary">{{ formatMessage(messages.themesDescription) }}</span>
				<div class="flex flex-wrap gap-2">
					<button
						v-for="theme in THEMES"
						:key="theme.id"
						type="button"
						class="flex cursor-pointer items-center gap-2 rounded-xl border-2 border-solid px-3 py-2 font-medium"
						:class="[
							isThemeActive(theme) ? 'border-contrast' : 'border-surface-5',
							theme.base ? 'text-white' : 'bg-surface-2 text-contrast',
						]"
						:style="theme.base ? { backgroundColor: theme.base } : undefined"
						:aria-pressed="isThemeActive(theme)"
						@click="applyTheme(theme)"
					>
						<span
							class="size-4 rounded-full"
							:class="theme.accent ? '' : 'bg-brand'"
							:style="theme.accent ? { backgroundColor: theme.accent } : undefined"
						/>
						{{ theme.id === 'default' ? formatMessage(messages.useDefault) : theme.name }}
					</button>
				</div>
			</div>

			<div class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.accent) }}</span>
				<span class="text-secondary">{{ formatMessage(messages.accentDescription) }}</span>
				<div class="flex flex-wrap items-center gap-2">
					<input
						type="color"
						:value="state.accent ?? '#3b82f6'"
						class="h-9 w-12 cursor-pointer rounded border-0 bg-transparent p-0"
						@input="setAccent(($event.target as HTMLInputElement).value)"
					/>
					<button
						v-for="color in ACCENT_PRESETS"
						:key="color"
						class="size-7 cursor-pointer rounded-full border-2 border-solid"
						:class="state.accent === color ? 'border-contrast' : 'border-transparent'"
						:style="{ backgroundColor: color }"
						:aria-label="color"
						@click="setAccent(color)"
					/>
					<Button type="outlined" :disabled="!state.accent" @click="setAccent(null)">
						{{ formatMessage(messages.useDefault) }}
					</Button>
				</div>
			</div>

			<div class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.base) }}</span>
				<span class="text-secondary">{{ formatMessage(messages.baseDescription) }}</span>
				<div class="flex flex-wrap items-center gap-2">
					<input
						type="color"
						:value="state.base ?? '#070a12'"
						class="h-9 w-12 cursor-pointer rounded border-0 bg-transparent p-0"
						@input="setBase(($event.target as HTMLInputElement).value)"
					/>
					<button
						v-for="color in BASE_PRESETS"
						:key="color"
						class="size-7 cursor-pointer rounded-full border-2 border-solid border-surface-5"
						:class="state.base === color ? '!border-contrast' : ''"
						:style="{ backgroundColor: color }"
						:aria-label="color"
						@click="setBase(color)"
					/>
					<Button type="outlined" :disabled="!state.base" @click="setBase(null)">
						{{ formatMessage(messages.useDefault) }}
					</Button>
				</div>
			</div>
		</div>

		<div>
			<Button type="outlined" color="red" @click="customization.resetAll()">
				{{ formatMessage(messages.resetAll) }}
			</Button>
		</div>
	</section>
</template>
