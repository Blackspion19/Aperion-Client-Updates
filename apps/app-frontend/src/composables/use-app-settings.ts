import { reactive, ref, watch } from 'vue'

export const DEFAULT_FEATURE_FLAGS = {
	project_background: false,
	page_path: false,
	server_project_qa: false,
	show_version_environment_column: false,
	server_ram_as_bytes_always_on: false,
	always_show_app_controls: false,
	show_sync_instances_update_modal: false,
	pride_fundraiser: true,
	i18n_debug: false,
	localhost_sign_in: false,
}

export type FeatureFlag = keyof typeof DEFAULT_FEATURE_FLAGS
type FeatureFlags = Record<FeatureFlag, boolean>

const syncBehaviorAcrossDevices = ref(false)
const syncFeaturesAcrossDevices = ref(false)
const featureFlags = reactive<FeatureFlags>({ ...DEFAULT_FEATURE_FLAGS })

function setBehaviorSyncAcrossDevices(enabled: boolean): void {
	syncBehaviorAcrossDevices.value = enabled
}

function setFeaturesSyncAcrossDevices(enabled: boolean): void {
	syncFeaturesAcrossDevices.value = enabled
}

function getFeatureFlag(key: FeatureFlag): boolean {
	return featureFlags[key] ?? DEFAULT_FEATURE_FLAGS[key]
}

const RESOURCEPACKS_SIDEBAR_KEY = 'aperion-show-resourcepacks-in-sidebar'

// Frontend-only setting, kept in localStorage (shown by default).
function loadShowResourcepacksInSidebar(): boolean {
	try {
		return window.localStorage.getItem(RESOURCEPACKS_SIDEBAR_KEY) !== 'false'
	} catch {
		return true
	}
}

const appSettings = reactive({
	showJumpIn: true,
	alwaysShowCopyDetails: false,
	hideInstalledModpacks: false,
	advancedFiltersCollapsed: true,
	dismissedPhotosensitivityFilterWarning: false,
	refocusOnGameClose: false,
	compactInstanceCards: false,
	showPlayTime: true,
	warnOnUnknownModpacks: true,
	skipNonEssentialWarnings: false,
	syncBehaviorAcrossDevices,
	syncFeaturesAcrossDevices,
	hideNametagSkinsPage: false,
	toggleSidebar: false,
	showFilesTabInInstances: true,
	showWorldsTabInInstances: true,
	showScreenshotsTabInInstances: false,
	showSkinSelectorInSidebar: true,
	showResourcepacksInSidebar: loadShowResourcepacksInSidebar(),
	nativeDecorations: false,
	devMode: false,
	featureFlags,
	setBehaviorSyncAcrossDevices,
	setFeaturesSyncAcrossDevices,
	getFeatureFlag,
})

watch(
	() => appSettings.showResourcepacksInSidebar,
	(value) => {
		try {
			window.localStorage.setItem(RESOURCEPACKS_SIDEBAR_KEY, String(value))
		} catch {
			// storage blocked
		}
	},
)

export function useAppSettings() {
	return appSettings
}
