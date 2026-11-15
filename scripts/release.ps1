# Aperion Release-Skript: alles in einem Schritt.
#
# Was es macht:
#   1. Traegt Repository-Adresse und Public Key in apps\app\tauri.updater.conf.json ein
#   2. Setzt die Version in apps\app-frontend\package.json
#   3. Fragt dein Schluessel-Passwort ab und baut die App (pnpm tauri build)
#   4. Erzeugt die Dateien fuer GitHub (release-upload\v<Version>\)
#
# Aufruf (im Hauptordner des Projekts):
#   .\scripts\release.ps1 -Version 1.0.3 -Notes "Was ist neu"
#
# Changelog im Launcher (apps\app-frontend\src\changelog.ts):
#   Version und Notes werden dort automatisch als neuer Eintrag oben eingetragen.
#   Mehrere Punkte trennst du mit Semikolon oder Zeilenumbruch. Optional mit Kategorie:
#     -Notes "new: Changelog-Knopf; improved: schnellerer Start; fix: Absturz beim Start"
#   Ohne Kategorie zaehlt ein Punkt als "New". Kategorien: new / improved / fix.
#   Ohne -Notes fragt das Skript die Punkte zeilenweise ab (leere Zeile beendet die Eingabe).
#
# Nur die Konfiguration pruefen/ausfuellen, ohne zu bauen:
#   .\scripts\release.ps1 -Version 1.0.3 -ConfigOnly

param(
	[Parameter(Mandatory = $true)][string]$Version,
	[string]$Repo = "Blackspion19/Aperion-Client-Updates",
	[string]$Notes = "",
	[string]$KeyPath = "$env:USERPROFILE\.tauri\aperion.key",
	[switch]$ConfigOnly
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-Step($text) { Write-Host ""; Write-Host "==> $text" -ForegroundColor Cyan }

# --- Hilfsfunktionen fuer den Changelog --------------------------------------------------
# Macht aus einer Eingabe wie "new: A; fix: B" einen sauberen Markdown-Text:
#   ## New / ## Improved / ## Fixed mit Listenpunkten
function ConvertTo-ChangelogBody([string]$Raw) {
	$sections = [ordered]@{ "New" = @(); "Improved" = @(); "Fixed" = @() }
	foreach ($part in ($Raw -split '\r?\n|;')) {
		$text = $part.Trim()
		$text = $text -replace '^[-*\u2022]+\s*', ''
		if ($text -eq "") { continue }
		$category = "New"
		if ($text -match '^(?i)(new|neu|feature|added|add)\s*:\s*(.+)$') {
			$text = $Matches[2]
		}
		elseif ($text -match '^(?i)(improved|improve|improvement|better|besser|verbessert|changed|geaendert|update|updated)\s*:\s*(.+)$') {
			$category = "Improved"; $text = $Matches[2]
		}
		elseif ($text -match '^(?i)(fix|fixed|bugfix|behoben|fehler)\s*:\s*(.+)$') {
			$category = "Fixed"; $text = $Matches[2]
		}
		$text = $text.Trim()
		if ($text -eq "") { continue }
		$text = $text.Substring(0, 1).ToUpper() + $text.Substring(1)
		$sections[$category] += $text
	}
	$blocks = @()
	foreach ($name in $sections.Keys) {
		$list = @($sections[$name])
		if ($list.Count -gt 0) {
			$blocks += ("## $name`n" + (($list | ForEach-Object { "- $_" }) -join "`n"))
		}
	}
	return ($blocks -join "`n`n")
}

# Traegt den Eintrag oben in changelog.ts ein. Gibt es die Version schon, wird ihr Eintrag ersetzt.
function Update-ChangelogFile([string]$Path, [string]$Version, [string]$Body) {
	$content = [System.IO.File]::ReadAllText($Path)
	$nl = "`n"
	if ($content.Contains("`r`n")) { $nl = "`r`n" }

	$marker = "export const CHANGELOG: ChangelogItem[] = ["
	$markerIdx = $content.IndexOf($marker)
	if ($markerIdx -lt 0) { throw "In $Path wurde die Zeile '$marker' nicht gefunden." }
	$lineEnd = $content.IndexOf("`n", $markerIdx)
	if ($lineEnd -lt 0) { throw "Unerwartetes Dateiende in $Path." }
	$insertAt = $lineEnd + 1

	# Sonderzeichen fuer ein JavaScript-Template-Literal absichern
	$escaped = $Body.Replace('\', '\\').Replace('`', '\`').Replace('${', '\${')
	$date = Get-Date -Format "yyyy-MM-dd"
	$entry = "`t{" + $nl + "`t`tversion: '$Version'," + $nl + "`t`tdate: '$date'," + $nl + "`t`tbody: ``" + $escaped + "``," + $nl + "`t}," + $nl

	$pattern = "\t\{\r?\n\t\tversion: '" + [regex]::Escape($Version) + "',[\s\S]*?\r?\n\t\},\r?\n"
	$m = ([regex]$pattern).Match($content)
	if ($m.Success) {
		$content = $content.Remove($m.Index, $m.Length).Insert($m.Index, $entry)
		Write-Host "  Vorhandener Eintrag fuer $Version wurde ersetzt."
	}
	else {
		$content = $content.Insert($insertAt, $entry)
	}
	[System.IO.File]::WriteAllText($Path, $content, $utf8NoBom)
}

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
	throw "Version muss wie 1.0.3 aussehen (ohne -local o. ae.), bekommen: '$Version'"
}

# --- 1. Updater-Konfiguration ausfuellen -------------------------------------------------
Write-Step "Updater-Konfiguration ausfuellen"

$pubPath = "$KeyPath.pub"
if (-not (Test-Path $pubPath)) { throw "Public Key nicht gefunden: $pubPath" }
if (-not (Test-Path $KeyPath)) { throw "Privater Schluessel nicht gefunden: $KeyPath" }

$pubkey = (Get-Content $pubPath -Raw).Trim()
if ($pubkey -match 'HIER' -or $pubkey.Length -lt 40) { throw "Der Inhalt von $pubPath sieht nicht wie ein Public Key aus." }

$endpoint = "https://github.com/$Repo/releases/latest/download/latest.json"

$confPath = Join-Path $root "apps\app\tauri.updater.conf.json"
if (-not (Test-Path $confPath)) { throw "Datei nicht gefunden: $confPath" }
$conf = [System.IO.File]::ReadAllText($confPath)

$conf = [regex]::Replace($conf, '"pubkey"\s*:\s*"[^"]*"', { param($m) '"pubkey": "' + $pubkey + '"' })
$conf = [regex]::Replace($conf, '"endpoints"\s*:\s*\[[^\]]*\]', { param($m) '"endpoints": ["' + $endpoint + '"]' })
[System.IO.File]::WriteAllText($confPath, $conf, $utf8NoBom)

# pruefen, ob alles gueltig ist
$check = $conf | ConvertFrom-Json
$u = $check.plugins.updater
if ($u.pubkey -ne $pubkey) { throw "pubkey konnte nicht gesetzt werden." }
if ($u.endpoints[0] -ne $endpoint) { throw "endpoint konnte nicht gesetzt werden." }
if ($conf -match 'DEIN-GITHUB|HIER-DEIN') { throw "In $confPath stehen noch Platzhalter." }
Write-Host "  Endpoint: $endpoint"
Write-Host "  Pubkey:   $($pubkey.Substring(0, 20))... (aus $pubPath)"

# --- 2. Version setzen ---------------------------------------------------------------------
Write-Step "Version auf $Version setzen"
$pkgPath = Join-Path $root "apps\app-frontend\package.json"
$pkg = [System.IO.File]::ReadAllText($pkgPath)
$rx = New-Object System.Text.RegularExpressions.Regex('"version"\s*:\s*"[^"]*"')
if (-not $rx.IsMatch($pkg)) { throw "Keine Version in $pkgPath gefunden." }
$pkg = $rx.Replace($pkg, '"version": "' + $Version + '"', 1)
[System.IO.File]::WriteAllText($pkgPath, $pkg, $utf8NoBom)
Write-Host "  $pkgPath"

# --- 2b. Changelog im Launcher aktualisieren -----------------------------------------------
Write-Step "Changelog aktualisieren"
$changelogPath = Join-Path $root "apps\app-frontend\src\changelog.ts"

if ((-not $Notes.Trim()) -and (-not $ConfigOnly)) {
	Write-Host "  Was ist neu in Version ${Version}? Eine Zeile pro Punkt, eine leere Zeile beendet die Eingabe."
	Write-Host "  Optional mit Kategorie: new: ...   improved: ...   fix: ...   (ohne Kategorie = new)"
	$lines = @()
	while ($true) {
		$line = Read-Host "  >"
		if ([string]::IsNullOrWhiteSpace($line)) { break }
		$lines += $line
	}
	$Notes = $lines -join "`n"
}

$notesText = $Notes
if ($Notes.Trim() -ne "") {
	$body = ConvertTo-ChangelogBody $Notes
	if ($body -eq "") { throw "Aus den Notes konnte kein Changelog-Text erzeugt werden." }
	if (-not (Test-Path $changelogPath)) {
		Write-Host "  Achtung: $changelogPath nicht gefunden - Changelog wird uebersprungen." -ForegroundColor Yellow
	}
	else {
		Update-ChangelogFile -Path $changelogPath -Version $Version -Body $body
		Write-Host "  Eingetragen in $changelogPath :"
		Write-Host ""
		($body -split "`n") | ForEach-Object { Write-Host "    $_" }
	}
	$notesText = $body
}
elseif (-not $ConfigOnly) {
	$answer = Read-Host "  Keine Notes eingegeben. Ohne Changelog-Eintrag weitermachen? (j/n)"
	if ($answer -ne "j") { throw "Abgebrochen: kein Changelog-Eintrag." }
	Write-Host "  Changelog wurde nicht geaendert." -ForegroundColor Yellow
}
else {
	Write-Host "  Keine Notes angegeben - Changelog wurde nicht geaendert."
}

if ($ConfigOnly) {
	Write-Host ""
	Write-Host "Konfiguration ist fertig (ConfigOnly). Es wurde nicht gebaut." -ForegroundColor Green
	return
}

# --- 3. Bauen ------------------------------------------------------------------------------
Write-Step "Bauen (das dauert einige Minuten)"

$secure = Read-Host "Passwort fuer den Schluessel aperion.key" -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try { $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) }
finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $KeyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plain

Push-Location (Join-Path $root "apps\app")
try {
	pnpm tauri build --config tauri.updater.conf.json
	if ($LASTEXITCODE -ne 0) { throw "Der Build ist mit Fehlercode $LASTEXITCODE fehlgeschlagen." }
}
finally {
	Pop-Location
	Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
	Remove-Item Env:\TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

# --- 4. Release-Dateien --------------------------------------------------------------------
Write-Step "Release-Dateien erzeugen"
& (Join-Path $PSScriptRoot "make-latest-json.ps1") -Version $Version -Repo $Repo -Notes $notesText

Write-Host ""
Write-Host "Naechste Schritte:" -ForegroundColor Green
Write-Host "  1. https://github.com/$Repo/releases -> 'Draft a new release'"
Write-Host "  2. Tag: v$Version"
Write-Host "  3. Die drei Dateien aus release-upload\v$Version\ hineinziehen"
Write-Host "  4. 'Set as a pre-release' NICHT anhaken, dann 'Publish release'"
