import { type ContentActionWarning, type ContentItem, defineMessages, useVIntl } from '@aperion/ui'
import { computed, type Ref } from 'vue'

import type { GameInstance } from '@/helpers/types'

export function useManagedContentPolicy(instance: Ref<GameInstance>) {
	const { formatMessage } = useVIntl()
	const isQuarantined = computed(() => instance.value.quarantined)

	function isManagedContent(_item: ContentItem) {
		return isQuarantined.value
	}

	function canMutateContent(item: ContentItem) {
		return !isManagedContent(item)
	}

	function canUpdateContent(item: ContentItem) {
		return (
			canMutateContent(item) && !!item.file_path && !!item.has_update && !!item.update_version_id
		)
	}

	function deleteWarning(items: ContentItem[]): ContentActionWarning | null {
		if (!items.some(isManagedContent)) return null
		return {
			admonitionHeader: formatMessage(messages.warningHeader),
			admonitionBody: formatMessage(
				items.length === 1 ? messages.deleteSingleBody : messages.deleteBulkBody,
			),
			actionLabel: formatMessage(
				items.length === 1 ? messages.deleteButton : messages.deleteManyButton,
				{ count: items.length },
			),
		}
	}

	function disableWarning(items: ContentItem[]): ContentActionWarning | null {
		if (!items.some(isManagedContent)) return null
		return {
			admonitionHeader: formatMessage(messages.warningHeader),
			admonitionBody: formatMessage(
				items.length === 1 ? messages.disableSingleBody : messages.disableBulkBody,
			),
			actionLabel: formatMessage(
				items.length === 1 ? messages.disableButton : messages.disableManyButton,
				{ count: items.length },
			),
		}
	}

	return {
		isQuarantined,
		isManagedContent,
		canMutateContent,
		canUpdateContent,
		deleteWarning,
		disableWarning,
	}
}

const messages = defineMessages({
	warningHeader: {
		id: 'content.locked-instance.warning-header',
		defaultMessage: 'This instance is locked',
	},
	deleteSingleBody: {
		id: 'content.locked-instance.delete-single-body',
		defaultMessage: 'Content in locked instances cannot be changed.',
	},
	deleteBulkBody: {
		id: 'content.locked-instance.delete-bulk-body',
		defaultMessage: 'Some selected projects are part of a locked instance and cannot be changed.',
	},
	deleteButton: { id: 'content.locked-instance.delete-button', defaultMessage: 'Delete anyway' },
	deleteManyButton: {
		id: 'content.locked-instance.delete-many-button',
		defaultMessage: 'Delete {count, number} projects anyway',
	},
	disableSingleBody: {
		id: 'content.locked-instance.disable-single-body',
		defaultMessage: 'Content in locked instances cannot be changed.',
	},
	disableBulkBody: {
		id: 'content.locked-instance.disable-bulk-body',
		defaultMessage:
			'Some selected projects are part of a locked instance and cannot be changed.',
	},
	disableButton: { id: 'content.locked-instance.disable-button', defaultMessage: 'Disable anyway' },
	disableManyButton: {
		id: 'content.locked-instance.disable-many-button',
		defaultMessage: 'Disable {count, number} projects anyway',
	},
})
