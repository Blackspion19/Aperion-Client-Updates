<template>
	<AperionIcon
		v-if="role === 'admin'"
		v-tooltip="formatMessage(messages.aperionTeamLabel)"
		role="img"
		class="size-4 shrink-0 !text-green"
	/>
	<ScaleIcon
		v-else-if="role === 'moderator'"
		v-tooltip="formatMessage(messages.moderatorLabel)"
		role="img"
		class="size-4 shrink-0 !text-orange"
	/>
</template>

<script setup lang="ts">
import type { Labrinth } from '@aperion/api-client'
import { AperionIcon, ScaleIcon } from '@aperion/assets'

import { defineMessages, useVIntl } from '#ui/composables/i18n'

defineProps<{
	role?: Labrinth.Users.v2.Role | null
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	aperionTeamLabel: {
		id: 'user.role.aperion-team',
		defaultMessage: 'Aperion Team',
	},
	moderatorLabel: {
		id: 'user.role.moderator',
		defaultMessage: 'Content Moderator',
	},
})
</script>
