$ErrorActionPreference = "Stop"

$InstallDir = Join-Path $env:LOCALAPPDATA "CodexPetLimitRings"
$Startup = Join-Path ([Environment]::GetFolderPath("Startup")) "CodexPetLimitRings.lnk"

Get-Process -Name "CodexPetLimitRings" -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item -LiteralPath $Startup -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $InstallDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "Codex Pet Limit Rings uninstalled"
