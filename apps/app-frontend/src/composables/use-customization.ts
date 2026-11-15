import { reactive } from 'vue'

/**
 * Personal customization of the launcher: a custom background image and
 * custom colors. Everything is stored locally.
 *
 * - colors and sliders: localStorage
 * - the background image itself: IndexedDB (too big for localStorage)
 * - applied as inline CSS variables on <html>, which win over the theme classes
 */

const STORAGE_KEY = 'aperion-customization'
const DB_NAME = 'aperion-customization'
const STORE = 'files'
const BACKGROUND_KEY = 'background'
const MAX_IMAGE_SIDE = 2560

type Stored = {
	accent: string | null
	base: string | null
	panelOpacity: number
	blur: number
	dim: number
}

const defaults: Stored = { accent: null, base: null, panelOpacity: 78, blur: 0, dim: 30 }

const state = reactive({
	...defaults,
	hasBackground: false,
})

let backgroundUrl: string | null = null
let initialized = false

// ---------- storage ----------

function load(): void {
	try {
		const raw = window.localStorage.getItem(STORAGE_KEY)
		if (!raw) return
		const parsed = JSON.parse(raw) as Partial<Stored>
		if (typeof parsed.accent === 'string' || parsed.accent === null) state.accent = parsed.accent ?? null
		if (typeof parsed.base === 'string' || parsed.base === null) state.base = parsed.base ?? null
		if (typeof parsed.panelOpacity === 'number') state.panelOpacity = parsed.panelOpacity
		if (typeof parsed.blur === 'number') state.blur = parsed.blur
		if (typeof parsed.dim === 'number') state.dim = parsed.dim
	} catch {
		// broken or blocked storage: use defaults
	}
}

function save(): void {
	try {
		const value: Stored = {
			accent: state.accent,
			base: state.base,
			panelOpacity: state.panelOpacity,
			blur: state.blur,
			dim: state.dim,
		}
		window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
	} catch {
		// storage blocked or full
	}
}

function openDb(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const request = indexedDB.open(DB_NAME, 1)
		request.onupgradeneeded = () => request.result.createObjectStore(STORE)
		request.onsuccess = () => resolve(request.result)
		request.onerror = () => reject(request.error)
	})
}

async function dbRun<T>(mode: IDBTransactionMode, run: (store: IDBObjectStore) => IDBRequest<T>) {
	const db = await openDb()
	try {
		return await new Promise<T>((resolve, reject) => {
			const request = run(db.transaction(STORE, mode).objectStore(STORE))
			request.onsuccess = () => resolve(request.result)
			request.onerror = () => reject(request.error)
		})
	} finally {
		db.close()
	}
}

// ---------- color helpers ----------

function cssToRgb(value: string): [number, number, number] | null {
	const element = document.createElement('span')
	element.style.color = value
	document.body.appendChild(element)
	const computed = getComputedStyle(element).color
	element.remove()
	const match = computed.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
	return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null
}

function hexToRgb(hex: string): [number, number, number] {
	const value = hex.replace('#', '')
	return [
		parseInt(value.slice(0, 2), 16) || 0,
		parseInt(value.slice(2, 4), 16) || 0,
		parseInt(value.slice(4, 6), 16) || 0,
	]
}

function rgbToHsl([r, g, b]: [number, number, number]): [number, number, number] {
	const rn = r / 255
	const gn = g / 255
	const bn = b / 255
	const max = Math.max(rn, gn, bn)
	const min = Math.min(rn, gn, bn)
	const l = (max + min) / 2
	if (max === min) return [0, 0, l * 100]
	const d = max - min
	const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
	let h = 0
	if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0)
	else if (max === gn) h = (bn - rn) / d + 2
	else h = (rn - gn) / d + 4
	return [h * 60, s * 100, l * 100]
}

function hslToCss(h: number, s: number, l: number): string {
	return `hsl(${Math.round(h)} ${Math.round(s)}% ${Math.round(l)}%)`
}

function isDarkMode(): boolean {
	const classes = document.documentElement.classList
	return classes.contains('dark-mode') || classes.contains('oled-mode') || classes.contains('retro-mode')
}

const SURFACES = [
	['--surface-1', 0],
	['--surface-1-5', 1.5],
	['--surface-2', 3],
	['--surface-2-5', 4.5],
	['--surface-3', 6],
	['--surface-4', 10],
	['--surface-5', 15],
] as const

const MANAGED = [
	...SURFACES.map(([name]) => name),
	'--color-brand',
	'--color-brand-highlight',
	'--color-brand-shadow',
	'--color-accent-contrast',
	'--brand-gradient-bg',
	'--brand-gradient-strong-bg',
	'--brand-gradient-border',
	'--brand-gradient-fade-out-color',
	'--splash-tint-top',
	'--splash-tint-bottom',
	'--splash-overlay',
	'--ap-bg-image',
	'--ap-bg-blur',
	'--ap-bg-dim',
	'--ap-panel-bg',
	'--ap-panel-raised-bg',
]

// ---------- applying ----------

function ensureBackgroundLayer(): void {
	if (document.getElementById('ap-custom-bg')) return
	const layer = document.createElement('div')
	layer.id = 'ap-custom-bg'
	const dim = document.createElement('div')
	dim.id = 'ap-custom-bg-dim'
	// The dim layer must come after the image layer to paint over it.
	document.body.prepend(dim)
	document.body.prepend(layer)
}

function ensureStyles(): void {
	// Rewritten on every call, so a live-reloaded module also updates the rules.
	let style = document.getElementById('ap-custom-style')
	if (!style) {
		style = document.createElement('style')
		style.id = 'ap-custom-style'
		document.head.appendChild(style)
	}
	style.textContent = `
#ap-custom-bg, #ap-custom-bg-dim { display: none; position: fixed; inset: 0; pointer-events: none; }
html.has-custom-bg #ap-custom-bg {
	display: block; z-index: 0;
	background-image: var(--ap-bg-image); background-size: cover; background-position: center;
	filter: blur(var(--ap-bg-blur, 0px)); transform: scale(1.06);
}
html.has-custom-bg #ap-custom-bg-dim {
	display: block; z-index: 0; background: rgba(0, 0, 0, var(--ap-bg-dim, 0.3));
}
html.has-custom-bg, html.has-custom-bg body { background: transparent !important; }
html.has-custom-bg #app { position: relative; z-index: 1; background: transparent !important; }
html.has-custom-bg .app-grid-layout { background-color: transparent !important; }
html.has-custom-bg .app-grid-navbar,
html.has-custom-bg .app-grid-statusbar { background-color: var(--ap-panel-raised-bg) !important; }
html.has-custom-bg .app-contents { background-color: var(--ap-panel-bg) !important; }
html.has-custom-bg .splash-screen .base-bg,
html.has-custom-bg .splash-screen .cube-bg { background: transparent !important; }
html.has-custom-bg .splash-screen .gradient-bg { background: var(--ap-panel-bg) !important; }
html.has-custom-bg .app-grid-layout .app-sidebar,
html.has-custom-bg .app-contents .app-sidebar,
html.has-custom-bg .app-sidebar {
	background-color: var(--ap-panel-bg) !important;
	background-image: var(--brand-gradient-bg) !important;
}
html.has-custom-bg .app-sidebar .app-sidebar-scrollable { background: transparent !important; }
`
}

function apply(): void {
	const html = document.documentElement
	ensureStyles()

	// Start clean so the theme's own values can be read again.
	for (const name of MANAGED) html.style.removeProperty(name)

	if (state.base && isDarkMode()) {
		const [h, s, l] = rgbToHsl(hexToRgb(state.base))
		for (const [name, offset] of SURFACES) {
			html.style.setProperty(name, hslToCss(h, s, Math.min(96, Math.max(0, l + offset))))
		}
	}

	if (state.accent) {
		const [r, g, b] = hexToRgb(state.accent)
		html.style.setProperty('--color-brand', state.accent)
		html.style.setProperty('--color-brand-highlight', `rgba(${r}, ${g}, ${b}, 0.25)`)
		html.style.setProperty('--color-brand-shadow', `rgba(${r}, ${g}, ${b}, 0.7)`)
		const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255
		html.style.setProperty('--color-accent-contrast', luminance > 0.6 ? '#000000' : '#ffffff')
	}

	// The sidebar and some panels get their tint from the theme's gradients,
	// which are fixed blue. Rebuild them from the chosen colors.
	if ((state.base && isDarkMode()) || state.accent) {
		const computed = getComputedStyle(html)
		const surface = (name: string, fallback: [number, number, number]) =>
			cssToRgb(computed.getPropertyValue(name).trim()) ?? fallback
		const s1 = surface('--surface-1', [7, 10, 18]).join(', ')
		const s2 = surface('--surface-2', [13, 18, 29]).join(', ')
		const s3 = surface('--surface-3', [20, 27, 43]).join(', ')
		const target = (
			state.accent ? hexToRgb(state.accent) : surface('--surface-5', [39, 54, 90])
		).join(', ')

		html.style.setProperty(
			'--brand-gradient-bg',
			`linear-gradient(0deg, rgba(${s2}, 0.3) 0%, rgba(${target}, 0.12) 100%)`,
		)
		html.style.setProperty(
			'--brand-gradient-strong-bg',
			`linear-gradient(270deg, rgb(${s1}) 10%, rgb(${s3}) 100%)`,
		)
		html.style.setProperty('--brand-gradient-border', `rgba(${target}, 0.18)`)
		// Startup screen
		html.style.setProperty('--splash-tint-top', `rgba(${target}, 0.28)`)
		html.style.setProperty('--splash-tint-bottom', `rgba(${s2}, 0.5)`)
		html.style.setProperty('--splash-overlay', `rgba(${s1}, 0.64)`)
		html.style.setProperty(
			'--brand-gradient-fade-out-color',
			`linear-gradient(to bottom, rgba(${s1}, 0), rgb(${s2}) 80%)`,
		)
	}

	const showBackground = state.hasBackground && backgroundUrl !== null
	html.classList.toggle('has-custom-bg', showBackground)
	if (showBackground) {
		ensureBackgroundLayer()
		html.style.setProperty('--ap-bg-image', `url("${backgroundUrl}")`)
		html.style.setProperty('--ap-bg-blur', `${state.blur}px`)
		html.style.setProperty('--ap-bg-dim', String(state.dim / 100))

		const computed = getComputedStyle(html)
		const base = cssToRgb(computed.getPropertyValue('--surface-1').trim()) ?? [7, 10, 18]
		const raised = cssToRgb(computed.getPropertyValue('--surface-3').trim()) ?? [20, 27, 43]
		const alpha = Math.min(1, Math.max(0, state.panelOpacity / 100))
		html.style.setProperty('--ap-panel-bg', `rgba(${base.join(', ')}, ${alpha})`)
		html.style.setProperty('--ap-panel-raised-bg', `rgba(${raised.join(', ')}, ${Math.min(1, alpha + 0.1)})`)
	}
}

function setBackgroundUrl(blob: Blob | null): void {
	if (backgroundUrl) URL.revokeObjectURL(backgroundUrl)
	backgroundUrl = blob ? URL.createObjectURL(blob) : null
}

async function shrinkImage(file: File): Promise<Blob> {
	const bitmap = await createImageBitmap(file)
	const scale = Math.min(1, MAX_IMAGE_SIDE / Math.max(bitmap.width, bitmap.height))
	const canvas = document.createElement('canvas')
	canvas.width = Math.max(1, Math.round(bitmap.width * scale))
	canvas.height = Math.max(1, Math.round(bitmap.height * scale))
	canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
	bitmap.close()
	const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.9))
	if (!blob) throw new Error('Image conversion failed')
	return blob
}

// ---------- public API ----------

export async function initCustomization(): Promise<void> {
	if (initialized) return
	initialized = true
	load()
	ensureStyles()
	try {
		const blob = (await dbRun('readonly', (store) => store.get(BACKGROUND_KEY))) as Blob | undefined
		if (blob) {
			setBackgroundUrl(blob)
			state.hasBackground = true
		}
	} catch {
		// no stored background
	}
	apply()

	// A theme change switches the class on <html>; derived colors follow it.
	new MutationObserver(() => apply()).observe(document.documentElement, {
		attributes: true,
		attributeFilter: ['class'],
	})
}

export function useCustomization() {
	function update(): void {
		save()
		apply()
	}

	return {
		state,
		update,
		async setBackground(file: File): Promise<void> {
			const blob = await shrinkImage(file)
			await dbRun('readwrite', (store) => store.put(blob, BACKGROUND_KEY))
			setBackgroundUrl(blob)
			state.hasBackground = true
			apply()
		},
		async removeBackground(): Promise<void> {
			await dbRun('readwrite', (store) => store.delete(BACKGROUND_KEY))
			setBackgroundUrl(null)
			state.hasBackground = false
			apply()
		},
		resetColors(): void {
			state.accent = null
			state.base = null
			update()
		},
		resetAll(): void {
			Object.assign(state, defaults)
			update()
		},
		previewUrl: (): string | null => backgroundUrl,
	}
}
