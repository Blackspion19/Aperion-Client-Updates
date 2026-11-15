<template>
	<section class="flex flex-col gap-3">
		<div class="flex flex-col gap-1">
			<h2 class="m-0 text-xl font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<p class="m-0 text-secondary">{{ formatMessage(messages.description) }}</p>
		</div>

		<div
			class="flex flex-col overflow-clip rounded-xl border border-solid border-surface-5 bg-button-bg"
		>
			<p v-if="accounts.length === 0" class="m-0 p-3 text-secondary">
				{{ formatMessage(messages.noAccounts) }}
			</p>
			<div
				v-for="account in accounts"
				:key="account.profile.id"
				class="flex items-center gap-1 border-0 border-b border-solid border-surface-5 last:border-b-0"
			>
				<button
					class="button-base flex min-w-0 flex-grow cursor-pointer items-center gap-2 overflow-clip border-0 bg-transparent p-3"
					@click="setAccount(account)"
				>
					<RadioButtonCheckedIcon
						v-if="account.profile.id === defaultUser"
						class="h-5 w-5 shrink-0 text-brand"
					/>
					<RadioButtonIcon v-else class="h-5 w-5 shrink-0 text-secondary" />
					<Avatar :src="savedHeads[account.profile.id]" size="28px" />
					<span
						class="min-w-0 truncate"
						:class="
							account.profile.id === defaultUser ? 'font-semibold text-contrast' : 'text-primary'
						"
					>
						{{ account.profile.name }}
					</span>
					<span v-if="account.profile.id === defaultUser" class="shrink-0 text-xs text-secondary">
						{{ formatMessage(messages.active) }}
					</span>
				</button>
				<IconButton
					v-tooltip="formatMessage(messages.remove)"
					type="quiet"
					color="red"
					:label="formatMessage(messages.remove)"
					class="mr-2"
					@click="removeAccount(account.profile.id)"
				>
					<TrashIcon />
				</IconButton>
			</div>
		</div>

		<div>
			<Button color="brand" :disabled="loginDisabled" @click="addAccount()">
				<SpinnerIcon v-if="loginDisabled" class="animate-spin" />
				<PlusIcon v-else />
				{{ formatMessage(messages.add) }}
			</Button>
		</div>
	</section>
</template>

<script setup lang="ts">
import {
	PlusIcon,
	RadioButtonCheckedIcon,
	RadioButtonIcon,
	SpinnerIcon,
	TrashIcon,
} from '@aperion/assets'
import {
	Avatar,
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	useVIntl,
} from '@aperion/ui'
import { onMounted, ref } from 'vue'

import { handleSevereError } from '@/composables/use-error.js'
import {
	get_default_user,
	login as login_flow,
	remove_user,
	set_default_user,
	users,
} from '@/helpers/auth'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

type MinecraftCredential = {
	profile: {
		id: string
		name: string
	}
}

// Dieselben lokal gespeicherten Koepfe wie in der Seitenleiste (AccountsCard).
const ACCOUNT_HEAD_PREFIX = 'aperion:account-head:'

const accounts = ref<MinecraftCredential[]>([])
const defaultUser = ref<string | undefined>()
const savedHeads = ref<Record<string, string>>({})
const loginDisabled = ref(false)

function loadSavedHeads() {
	try {
		const result: Record<string, string> = {}
		for (const account of accounts.value) {
			const value = localStorage.getItem(ACCOUNT_HEAD_PREFIX + account.profile.id)
			if (value) result[account.profile.id] = value
		}
		savedHeads.value = result
	} catch {
		savedHeads.value = {}
	}
}

async function refresh() {
	defaultUser.value = await get_default_user().catch(handleError)
	const userList = await users().catch(handleError)
	accounts.value = Array.isArray(userList) ? [...userList] : []
	accounts.value.sort((a, b) => (a.profile?.name ?? '').localeCompare(b.profile?.name ?? ''))
	loadSavedHeads()
}

function notifyChanged() {
	window.dispatchEvent(new CustomEvent('aperion:accounts-changed'))
}

async function setAccount(account: MinecraftCredential) {
	if (account.profile.id === defaultUser.value) return
	await set_default_user(account.profile.id).catch(handleError)
	await refresh()
	notifyChanged()
}

async function addAccount() {
	loginDisabled.value = true
	try {
		const loggedIn = await login_flow().catch(handleSevereError)
		if (loggedIn) {
			await set_default_user(loggedIn.profile.id).catch(handleError)
			await refresh()
			notifyChanged()
		}
	} finally {
		loginDisabled.value = false
	}
}

async function removeAccount(id: string) {
	await remove_user(id).catch(handleError)
	try {
		localStorage.removeItem(ACCOUNT_HEAD_PREFIX + id)
	} catch {
		// nicht kritisch
	}
	await refresh()
	if (!accounts.value.some((account) => account.profile.id === defaultUser.value)) {
		if (accounts.value.length > 0) {
			await set_default_user(accounts.value[0].profile.id).catch(handleError)
			await refresh()
		}
	}
	notifyChanged()
}

onMounted(refresh)

const messages = defineMessages({
	title: {
		id: 'app.settings.profile.minecraft-accounts.title',
		defaultMessage: 'Minecraft accounts',
	},
	description: {
		id: 'app.settings.profile.minecraft-accounts.description',
		defaultMessage: 'The accounts you are signed in with. The selected account is used to play.',
	},
	noAccounts: {
		id: 'app.settings.profile.minecraft-accounts.none',
		defaultMessage: 'No Minecraft account signed in yet.',
	},
	active: {
		id: 'app.settings.profile.minecraft-accounts.active',
		defaultMessage: 'Active',
	},
	add: {
		id: 'app.settings.profile.minecraft-accounts.add',
		defaultMessage: 'Add account',
	},
	remove: {
		id: 'app.settings.profile.minecraft-accounts.remove',
		defaultMessage: 'Remove account',
	},
})
</script>
