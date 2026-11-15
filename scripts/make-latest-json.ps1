# Erzeugt den Release-Ordner (Installer + .sig + latest.json) fuer GitHub.
# Aufruf (im Hauptordner des Projekts):
#   .\scripts\make-latest-json.ps1 -Version 1.0.1 -Repo "DEIN-NAME/DEIN-REPO" -Notes "Was ist neu"
param(
	[Parameter(Mandatory = $true)][string]$Version,
	[Parameter(Mandatory = $true)][string]$Repo,
	[string]$Notes = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$nsis = Join-Path $root "target\release\bundle\nsis"

if (-not (Test-Path $nsis)) { throw "Ordner nicht gefunden: $nsis - der Build ist nicht (vollstaendig) durchgelaufen." }
$exe = Get-ChildItem $nsis -Filter "*_${Version}_x64-setup.exe" | Select-Object -First 1
if (-not $exe) {
	Write-Host "Gefundene Dateien in ${nsis}:" -ForegroundColor Yellow
	Get-ChildItem $nsis | ForEach-Object { Write-Host "  $($_.Name)" }
	throw "Kein Installer fuer Version $Version gefunden. Steht in apps\app-frontend\package.json genau '$Version' (ohne -local)?"
}
$sigFile = "$($exe.FullName).sig"
if (-not (Test-Path $sigFile)) { throw "Signaturdatei fehlt: $sigFile (TAURI_SIGNING_PRIVATE_KEY beim Bauen gesetzt?)" }

$outDir = Join-Path $root "release-upload\v$Version"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# GitHub ersetzt Leerzeichen in Dateinamen -> wir nehmen von Anfang an Bindestriche
$safeName = $exe.Name -replace " ", "-"
Copy-Item $exe.FullName (Join-Path $outDir $safeName) -Force
Copy-Item $sigFile (Join-Path $outDir "$safeName.sig") -Force

$signature = (Get-Content $sigFile -Raw).Trim()
$json = [ordered]@{
	version  = $Version
	notes    = $Notes
	pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
	platforms = [ordered]@{
		"windows-x86_64" = [ordered]@{
			signature = $signature
			url       = "https://github.com/$Repo/releases/download/v$Version/$safeName"
		}
	}
} | ConvertTo-Json -Depth 5

[System.IO.File]::WriteAllText((Join-Path $outDir "latest.json"), $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host ""
Write-Host "Fertig. Diese Dateien bei GitHub hochladen:" -ForegroundColor Green
Get-ChildItem $outDir | ForEach-Object { Write-Host "  $($_.FullName)" }
