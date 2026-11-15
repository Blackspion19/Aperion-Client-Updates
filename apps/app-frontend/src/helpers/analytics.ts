// Aperion sendet keine Analytics-Daten. Diese Datei behaelt nur die bisherige
// Schnittstelle, damit alle Aufrufe im Code (trackEvent usw.) weiter funktionieren.
// Es wird nichts geladen, gespeichert oder gesendet.

export type AnalyticsEvent = string

export const initAnalytics = () => {}

export const debugAnalytics = () => {}

export const optOutAnalytics = () => {}

export const optInAnalytics = () => {}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
export const trackEvent = (_eventName: AnalyticsEvent, _properties?: Record<string, unknown>) => {}
