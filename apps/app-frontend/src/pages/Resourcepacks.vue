<script setup lang="ts">
import { LayersIcon } from '@aperion/assets'
import { defineMessages, useVIntl } from '@aperion/ui'
import { onActivated } from 'vue'

import ResourcepacksPage from '@/components/ui/resourcepacks-page/index.vue'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

defineOptions({ name: 'ResourcepacksPage' })

const { formatMessage } = useVIntl()
const messages = defineMessages({
	resourcepacks: { id: 'app.resourcepacks.heading', defaultMessage: 'Resource packs' },
})
const breadcrumb = useRootBreadcrumb({
	slot: 'root',
	id: 'resourcepacks',
	label: formatMessage(messages.resourcepacks),
	to: '/resourcepacks',
	visual: { type: 'icon', component: LayersIcon },
})
onActivated(breadcrumb.reset)
</script>

<template>
	<div class="box-border h-full overflow-y-auto p-6">
		<ResourcepacksPage show-heading />
	</div>
</template>
