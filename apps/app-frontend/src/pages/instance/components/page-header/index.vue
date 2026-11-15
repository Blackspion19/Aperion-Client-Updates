<template>
	<PageHeader :title="instance.name">
		<template #leading>
			<Avatar
				:src="iconSrc"
				:alt="instance.name"
				size="64px"
				:tint-by="instance.id"
				pad-transparent-corners
			/>
		</template>

		<template v-if="instance.quarantined" #badges>
			<PageHeaderBadgeItem
				:icon="LockIcon"
				aria-label="Locked instance information"
				class="!border-orange !bg-highlight-orange !text-orange"
			>
				Locked
			</PageHeaderBadgeItem>
		</template>

		<template #metadata>
			<InstanceHeaderServerMetadata
				v-if="isServerInstance"
				:loading-server-ping="loadingServerPing"
				:players-online="playersOnline"
				:status-online="statusOnline"
				:ping="ping"
				:minecraft-server="minecraftServer"
				:show-instance-play-time="showInstancePlayTime"
				:playtime-label="playtimeLabel ?? formatMessage(messages.neverPlayed)"
			/>
			<PageHeaderMetadata v-else>
				<PageHeaderMetadataItem
					:icon="TagIcon"
					:icon-props="{ tag: loaderDisplayName, enforceType: 'loader' }"
					tooltip="Mod loader and Minecraft version"
				>
					{{ loaderLabel }}
				</PageHeaderMetadataItem>
				<PageHeaderMetadataItem
					v-if="showInstancePlayTime && playtimeLabel"
					:icon="TimerIcon"
					tooltip="Total playtime"
				>
					{{ playtimeLabel }}
				</PageHeaderMetadataItem>
				<PageHeaderMetadataTimeItem
					v-if="instance.last_played"
					:icon="ClockIcon"
					:date="instance.last_played"
					:label="formatMessage(messages.lastPlayed)"
				/>
				<PageHeaderMetadataItem v-else :icon="ClockIcon" tooltip="Last played">
					{{ formatMessage(messages.neverPlayed) }}
				</PageHeaderMetadataItem>
			</PageHeaderMetadata>
		</template>

		<template #actions>
			<PageHeaderActions>
				<Button
					v-if="isInstalling"
					type="colored"
					color="brand"
					size="xl"
					native-type="button"
					disabled
				>
					{{ formatMessage(commonMessages.installingLabel) }}
				</Button>
				<Button
					v-else-if="playing"
					type="colored"
					color="red"
					size="xl"
					native-type="button"
					:disabled="stopping"
					@click="emit('stop')"
				>
					<StopCircleIcon />
					{{
						stopping ? formatMessage(messages.stopping) : formatMessage(commonMessages.stopButton)
					}}
				</Button>
				<Button
					v-else-if="instance.quarantined"
					v-tooltip="formatMessage(messages.lockedPlayTooltip)"
					type="colored"
					color="brand"
					size="xl"
					native-type="button"
					disabled
				>
					<PlayIcon />
					{{ formatMessage(commonMessages.playButton) }}
				</Button>
				<Button
					v-else-if="instance.install_stage !== 'installed'"
					type="colored"
					color="brand"
					size="xl"
					native-type="button"
					@click="emit('repair')"
				>
					<DownloadIcon />
					{{ formatMessage(messages.repair) }}
				</Button>
				<SplitButton
					v-else-if="!loading && isServerInstance"
					type="colored"
					color="brand"
					size="xl"
					:options="serverPlayOptions"
					:menu-label="formatMessage(messages.launchInstance)"
					@click="emit('playServer')"
				>
					<PlayIcon />
					{{ formatMessage(commonMessages.playButton) }}
				</SplitButton>
				<Button
					v-else-if="!loading"
					type="colored"
					color="brand"
					size="xl"
					native-type="button"
					@click="emit('play')"
				>
					<PlayIcon />
					{{ formatMessage(commonMessages.playButton) }}
				</Button>
				<Button v-else type="colored" color="brand" size="xl" native-type="button" disabled>{{
					formatMessage(messages.starting)
				}}</Button>

				<Button
					v-if="!isInstalling"
					v-tooltip="ingameTooltip"
					:type="ingameEnabled ? 'colored' : 'outlined'"
					:color="ingameEnabled ? 'brand' : undefined"
					size="xl"
					native-type="button"
					:disabled="ingameBusy || playing || (!ingameEnabled && !!ingameBlockedReason)"
					@click="toggleIngame"
				>
					{{
						ingameEnabled
							? formatMessage(messages.ingameOn)
							: formatMessage(messages.ingameOff)
					}}
				</Button>
				<IconButton
					v-tooltip="formatMessage(messages.instanceSettings)"
					size="xl"
					:label="formatMessage(messages.instanceSettings)"
					native-type="button"
					@click="emit('settings')"
				>
					<SettingsIcon />
				</IconButton>
				<IconButton
					v-if="showSchematics"
					v-tooltip="formatMessage(messages.schematics)"
					size="xl"
					:label="formatMessage(messages.schematics)"
					native-type="button"
					@click="emit('schematics')"
				>
					<LitematicaIcon />
				</IconButton>
				<TeleportOverflowMenu
					type="quiet"
					size="xl"
					:label="formatMessage(messages.moreActions)"
					:tooltip="formatMessage(messages.moreActions)"
					:options="moreActions"
				>
					<MoreVerticalIcon />
				</TeleportOverflowMenu>
			</PageHeaderActions>
		</template>
	</PageHeader>
</template>

<script setup lang="ts">
import type { Labrinth } from '@aperion/api-client'
import {
	ClockIcon,
	DownloadIcon,
	ExternalIcon,
	FolderOpenIcon,
	LockIcon,
	MoreVerticalIcon,
	PackageIcon,
	PlayIcon,
	SettingsIcon,
	StopCircleIcon,
	TimerIcon,
} from '@aperion/assets'
import { Button, IconButton, SplitButton, TeleportOverflowMenu } from '@aperion/ui'
import {
	Avatar,
	type ButtonMenuOption,
	commonMessages,
	defineMessages,
	formatLoaderLabel,
	PageHeader,
	PageHeaderActions,
	PageHeaderBadgeItem,
	PageHeaderMetadata,
	PageHeaderMetadataItem,
	PageHeaderMetadataTimeItem,
	type ServerLoader,
	TagIcon,
	injectNotificationManager,
	useVIntl,
} from '@aperion/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { aperion_ingame_info, aperion_ingame_prefetch } from '@/helpers/aperion-ingame'
import { edit } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types'

import LitematicaIcon from '@/components/ui/schematics/LitematicaIcon.vue'

import { instanceKeys } from '../../query-options'
import InstanceHeaderServerMetadata from './instance-page-header-server-metadata.vue'

const messages = defineMessages({
	createShortcut: {
		id: 'instance.action.create-shortcut',
		defaultMessage: 'Create shortcut',
	},
	exportModpack: {
		id: 'instance.action.export-modpack',
		defaultMessage: 'Export modpack',
	},
	instanceSettings: {
		id: 'instance.action.settings',
		defaultMessage: 'Instance settings',
	},
	ingameOn: {
		id: 'instance.action.ingame-on',
		defaultMessage: 'Ingame client: On',
	},
	ingameOff: {
		id: 'instance.action.ingame-off',
		defaultMessage: 'Ingame client: Off',
	},
	ingameTurnOn: {
		id: 'instance.action.ingame-turn-on',
		defaultMessage: 'Turn on the Aperion ingame client for this instance',
	},
	ingameTurnOff: {
		id: 'instance.action.ingame-turn-off',
		defaultMessage: 'Turn off the Aperion ingame client for this instance',
	},
	ingameNeedsFabric: {
		id: 'instance.action.ingame-needs-fabric',
		defaultMessage: 'The ingame client needs a Fabric instance',
	},
	ingameNoBuild: {
		id: 'instance.action.ingame-no-build',
		defaultMessage: 'No ingame client available for Minecraft {version} yet',
	},
	ingameWhilePlaying: {
		id: 'instance.action.ingame-while-playing',
		defaultMessage: 'Stop the game first to change this',
	},
	launchInstance: {
		id: 'instance.action.launch-instance',
		defaultMessage: 'Launch instance',
	},
	moreActions: {
		id: 'instance.action.more-actions',
		defaultMessage: 'More actions',
	},
	neverPlayed: {
		id: 'instance.playtime.never-played',
		defaultMessage: 'Never played',
	},
	lastPlayed: {
		id: 'instance.last-played',
		defaultMessage: 'Last played',
	},
	openFolder: {
		id: 'instance.action.open-folder',
		defaultMessage: 'Open folder',
	},
	schematics: {
		id: 'instance.action.schematics',
		defaultMessage: 'Schematics',
	},
	repair: {
		id: 'instance.action.repair',
		defaultMessage: 'Repair',
	},
	lockedPlayTooltip: {
		id: 'instance.locked.play-tooltip',
		defaultMessage: 'This instance has been locked',
	},
	starting: {
		id: 'instance.action.starting',
		defaultMessage: 'Starting...',
	},
	stopping: {
		id: 'instance.action.stopping',
		defaultMessage: 'Stopping...',
	},
})

const props = withDefaults(
	defineProps<{
		instance: GameInstance
		iconSrc?: string | null
		isServerInstance?: boolean
		showSchematics?: boolean
		showInstancePlayTime?: boolean
		timePlayed?: number
		playing?: boolean
		loading?: boolean
		stopping?: boolean
		loadingServerPing?: boolean
		playersOnline?: number
		statusOnline?: boolean
		ping?: number
		minecraftServer?: Labrinth.Projects.v3.Project['minecraft_server']
	}>(),
	{
		iconSrc: null,
		isServerInstance: false,
		showSchematics: false,
		showInstancePlayTime: false,
		timePlayed: 0,
		playing: false,
		loading: false,
		stopping: false,
		loadingServerPing: false,
		playersOnline: undefined,
		statusOnline: false,
		ping: undefined,
		minecraftServer: undefined,
	},
)

const emit = defineEmits<{
	repair: []
	stop: []
	play: []
	playServer: []
	settings: []
	openFolder: []
	export: []
	createShortcut: []
	schematics: []
}>()

const installingStages = [
	'installing',
	'pack_installing',
	'pack_installed',
	'not_installed',
	'minecraft_installing',
]

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()

// Aperion ingame client toggle (off by default for every instance)
const ingameBusy = ref(false)
const ingameEnabled = computed(() => props.instance.aperion_ingame === true)
const ingameQuery = useQuery({
	queryKey: computed(() => ['aperion-ingame', 'info', props.instance.game_version] as const),
	queryFn: () => aperion_ingame_info(props.instance.game_version),
	staleTime: 5 * 60 * 1000,
	retry: false,
})
const ingameBlockedReason = computed(() => {
	if (props.instance.loader !== 'fabric') return formatMessage(messages.ingameNeedsFabric)
	if (ingameQuery.data.value && !ingameQuery.data.value.available) {
		return formatMessage(messages.ingameNoBuild, { version: props.instance.game_version })
	}
	return undefined
})
const ingameTooltip = computed(() => {
	if (props.playing) return formatMessage(messages.ingameWhilePlaying)
	if (ingameEnabled.value) return formatMessage(messages.ingameTurnOff)
	return ingameBlockedReason.value ?? formatMessage(messages.ingameTurnOn)
})

async function toggleIngame() {
	if (ingameBusy.value) return
	ingameBusy.value = true
	const next = !ingameEnabled.value
	try {
		await edit(props.instance.id, { aperion_ingame: next })
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: instanceKeys.detail(props.instance.id) }),
			queryClient.invalidateQueries({ queryKey: instanceKeys.list() }),
		])
		if (next) {
			// Fetch the jar in the background so the next start does not have to wait.
			aperion_ingame_prefetch(props.instance.game_version)
				.then(() => ingameQuery.refetch())
				.catch(() => {})
		}
	} catch (error) {
		handleError(error as Error)
	} finally {
		ingameBusy.value = false
	}
}

const isInstalling = computed(() => installingStages.includes(props.instance.install_stage))
const loaderDisplayName = computed(() => formatLoaderLabel(props.instance.loader) as ServerLoader)
const loaderLabel = computed(() =>
	[loaderDisplayName.value, props.instance.game_version].filter(Boolean).join(' '),
)
const playtimeLabel = computed(() => {
	const seconds = Math.floor(props.timePlayed)
	if (seconds <= 0) {
		return undefined
	}

	const hours = Math.floor(seconds / 3600)
	if (hours >= 1) {
		return `${hours} hour${hours > 1 ? 's' : ''}`
	}

	const minutes = Math.floor(seconds / 60)
	if (minutes >= 1) {
		return `${minutes} minute${minutes > 1 ? 's' : ''}`
	}

	return `${seconds} second${seconds === 1 ? '' : 's'}`
})
const serverPlayOptions = computed<ButtonMenuOption[]>(() => [
	{
		id: 'launch_instance',
		label: formatMessage(messages.launchInstance),
		icon: PlayIcon,
		action: () => emit('play'),
	},
])
const moreActions = computed<ButtonMenuOption[]>(() => {
	const actions: ButtonMenuOption[] = [
		{
			id: 'open-folder',
			label: formatMessage(messages.openFolder),
			icon: FolderOpenIcon,
			action: () => emit('openFolder'),
		},
	]

	if (!props.instance.quarantined) {
		actions.push(
			{
				id: 'export-mrpack',
				label: formatMessage(messages.exportModpack),
				icon: PackageIcon,
				action: () => emit('export'),
			},
			{
				id: 'create-shortcut',
				label: formatMessage(messages.createShortcut),
				icon: ExternalIcon,
				action: () => emit('createShortcut'),
			},
		)
	}

	return actions
})
</script>
