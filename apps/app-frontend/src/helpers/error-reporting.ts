import type { App } from 'vue'
import type { Router } from 'vue-router'

// Aperion sendet keine Fehlerberichte an Dritte. Die Funktion bleibt bestehen,
// damit main.js unveraendert funktioniert, tut aber nichts.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function setupErrorReporting(_app: App, _router: Router): void {}
