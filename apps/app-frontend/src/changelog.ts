// Changelog der Aperion App.
//
// So pflegst du ihn: Vor jedem Release oben in der Liste einen neuen Eintrag einfügen.
// Neueste Version zuerst. `date` im Format JJJJ-MM-TT. `body` ist Markdown:
// Überschriften mit "## ..." und Listenpunkte mit "- ...". Die Texte sind englisch.

export type ChangelogItem = {
	version: string
	date: string
	body: string
}

export const CHANGELOG: ChangelogItem[] = [
	{
		version: '1.1.5',
		date: '2026-10-17',
		body: `## New
- New **Changelog** button in the top bar: see what's new in every version.

## Improved
- No more ads in the app.
- No analytics or error data is sent to third parties anymore.
- Player heads are now generated inside the app instead of being loaded from a third party.
- The app is now much leaner.`,
	},
]
