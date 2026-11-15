<script setup lang="ts">
import { CheckIcon, PackageIcon } from '@aperion/assets'
import { defineMessages, injectNotificationManager, Toggle, useVIntl } from '@aperion/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, onMounted, ref } from 'vue'

import { list as listInstances } from '@/helpers/instance'
import {
	deploy_resourcepack_to_instance,
	is_resourcepack_deployed_to_instance,
	remove_resourcepack_from_instance,
	type ResourcePackMetadata,
} from '@/helpers/resourcepack'
import type { GameInstance } from '@/helpers/types'

const props = defineProps<{
	pack: ResourcePackMetadata
}>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	heading: {
		id: 'app.resourcepacks.deploy.heading',
		defaultMessage: 'Use in instances',
	},
	description: {
		id: 'app.resourcepacks.deploy.description',
		defaultMessage:
			'Deploying copies this pack into an instance\u2019s resource pack folder. Only instances on {version} are shown.',
	},
	empty: {
		id: 'app.resourcepacks.deploy.empty',
		defaultMessage: 'No instances on {version} yet.',
	},
})

const instances = ref<GameInstance[]>([])
const deployedState = ref<Record<string, boolean>>({})
const busy = ref(new Set<string>())
const loading = ref(true)

const matchingInstances = computed(() =>
	instances.value.filter((instance) => instance.game_version === props.pack.game_version),
)

async function refresh() {
	loading.value = true
	try {
		instances.value = await listInstances()
		const relevant = instances.value.filter(
			(instance) => instance.game_version === props.pack.game_version,
		)
		const entries = await Promise.all(
			relevant.map(async (instance) => {
				try {
					return [
						instance.id,
						await is_resourcepack_deployed_to_instance(props.pack.id, instance.id),
					] as const
				} catch {
					return [instance.id, false] as const
				}
			}),
		)
		deployedState.value = Object.fromEntries(entries)
	} catch (e) {
		handleError(e)
	} finally {
		loading.value = false
	}
}

async function toggle(instance: GameInstance, value: boolean) {
	busy.value.add(instance.id)
	try {
		if (value) {
			await deploy_resourcepack_to_instance(props.pack.id, instance.id)
		} else {
			await remove_resourcepack_from_instance(props.pack.id, instance.id)
		}
		deployedState.value = { ...deployedState.value, [instance.id]: value }
	} catch (e) {
		handleError(e)
	} finally {
		busy.value.delete(instance.id)
	}
}

onMounted(refresh)
</script>

<template>
	<div class="flex flex-col gap-2 rounded-2xl border border-solid border-surface-4 bg-bg-raised p-4">
		<div class="flex items-center gap-2">
			<CheckIcon class="size-5 text-brand" />
			<h2 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.heading) }}</h2>
		</div>
		<p class="m-0 text-sm text-secondary">
			{{ formatMessage(messages.description, { version: pack.game_version }) }}
		</p>

		<p v-if="!loading && matchingInstances.length === 0" class="m-0 text-sm text-secondary">
			{{ formatMessage(messages.empty, { version: pack.game_version }) }}
		</p>

		<div v-else class="flex flex-col divide-y divide-solid divide-surface-4">
			<div
				v-for="instance in matchingInstances"
				:key="instance.id"
				class="flex items-center gap-3 py-2 first:pt-0 last:pb-0"
			>
				<img
					v-if="instance.icon_path"
					:src="convertFileSrc(instance.icon_path)"
					class="size-8 shrink-0 rounded-md object-cover"
					alt=""
				/>
				<PackageIcon v-else class="size-8 shrink-0 text-secondary" />
				<span class="min-w-0 flex-1 truncate text-contrast">{{ instance.name }}</span>
				<Toggle
					:model-value="deployedState[instance.id] ?? false"
					:disabled="busy.has(instance.id)"
					@update:model-value="(value) => toggle(instance, value)"
				/>
			</div>
		</div>
	</div>
</template>
