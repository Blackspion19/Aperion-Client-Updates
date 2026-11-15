import type { AbstractAperionClient } from '@aperion/api-client'
import type { AbstractPopupNotificationManager, AbstractWebNotificationManager } from '@aperion/ui'

import type { InstanceIconConfig } from '@/helpers/types'

import type { AppEvents } from './app-events'
import { setupOnboardingChecklistProvider } from './onboarding-checklist'
import { setupCreationModal } from './setup/creation-modal'
import { setupFileDropProvider } from './setup/file-drop'
import { setupFilePickerProvider } from './setup/file-picker'
import { setupImageViewerEditorProvider } from './setup/image-viewer-editor'
import { setupInstanceImportProvider } from './setup/instance-import'
import { setupTagsProvider } from './setup/tags'
import { setupUserCountryProvider } from './setup/user-country'

export function setupProviders(
	client: AbstractAperionClient,
	notificationManager: AbstractWebNotificationManager,
	_popupNotificationManager: AbstractPopupNotificationManager,
	appEvents: AppEvents,
	getGeneratedIconConfig?: (iconPath: string) => InstanceIconConfig | null,
) {
	setupUserCountryProvider(client)
	const tags = setupTagsProvider(notificationManager)
	setupFileDropProvider()
	setupFilePickerProvider()
	setupImageViewerEditorProvider()
	setupInstanceImportProvider(notificationManager)
	const onboardingChecklist = setupOnboardingChecklistProvider(appEvents)

	return {
		...setupCreationModal(notificationManager, getGeneratedIconConfig),
		onboardingChecklist,
		tags,
	}
}
