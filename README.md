# Aperion App

Aperion ist ein Minecraft-Launcher für Windows. Er basiert auf der Open-Source-[Modrinth App](https://github.com/modrinth/code) und verwaltet Instanzen, Mods, Modpacks, Resourcepacks und Welten.

> Aperion ist ein unabhängiges Projekt und steht in keiner Verbindung zu Modrinth oder Rinth, Inc. Mods und Modpacks werden über die öffentliche Schnittstelle von Modrinth geladen.

## Download

Die aktuelle Version gibt es unter [Releases](https://github.com/Blackspion19/Aperion-Client-Updates/releases/latest). Die App aktualisiert sich selbst, sobald eine neue Version erscheint.

## Was anders ist als in der Modrinth App

- Keine Werbung.
- Keine Analyse- oder Fehlerdaten an Dritte.
- Spielerköpfe werden lokal erzeugt, ohne Abruf bei Drittanbietern.
- Oberfläche auf Deutsch und Englisch.
- Überarbeitete Resourcepack-Verwaltung.

## Entwicklung

Voraussetzungen: [Node.js](https://nodejs.org), [pnpm](https://pnpm.io), [Rust](https://rustup.rs) und die [Tauri-Voraussetzungen für Windows](https://tauri.app/start/prerequisites/).

```powershell
pnpm install
cd apps\app
pnpm tauri dev --config tauri.show-window.conf.json
```

Einen Release baut das Skript `scripts\release.ps1`:

```powershell
.\scripts\release.ps1 -Version 1.1.4 -Notes "Was ist neu"
```

Es setzt die Version, baut die App, signiert sie und legt die Dateien für GitHub unter `release-upload\` ab.

## Lizenz und Quellcode

Der Quellcode der App steht unter der **GNU General Public License, Version 3 (nur)**, siehe [LICENSE](apps/app/LICENSE). Den Quellcode der jeweils veröffentlichten Version findest du hier: **<LINK-ZUM-QUELLCODE-EINTRAGEN>**.

Hinweise zu Herkunft, Änderungen und Marken stehen in [NOTICE.md](NOTICE.md). Weitere Lizenzangaben stehen in den Dateien `LICENSE` und `COPYING.md` der einzelnen Pakete.
