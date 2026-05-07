$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Manifest = Join-Path $Root "tools\rust\Cargo.toml"
$RustExe = Join-Path $Root "tools\rust\target\release\codex-pet-limit-rings.exe"
$InstallDir = Join-Path $env:LOCALAPPDATA "CodexPetLimitRings"
$Exe = Join-Path $InstallDir "CodexPetLimitRings.exe"
$Startup = Join-Path ([Environment]::GetFolderPath("Startup")) "CodexPetLimitRings.lnk"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found. Install Rust before building Codex Pet Limit Rings from source."
}

if (-not (Test-Path -LiteralPath $Manifest)) {
    throw "Rust project was not found: $Manifest"
}

Get-Process -Name "CodexPetLimitRings", "codex-pet-limit-rings" -ErrorAction SilentlyContinue | Stop-Process -Force
cargo build --manifest-path $Manifest --release | Out-Null

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -LiteralPath $RustExe -Destination $Exe -Force

$Shell = New-Object -ComObject WScript.Shell
$Shortcut = $Shell.CreateShortcut($Startup)
$Shortcut.TargetPath = $Exe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "Codex Pet Limit Rings"
$Shortcut.Save()

Start-Process -FilePath $Exe -ArgumentList "--show-without-pet" -WindowStyle Hidden
Start-Sleep -Milliseconds 800

Write-Host "Codex Pet Limit Rings installed at $InstallDir"
Write-Host "Startup shortcut: $Startup"
& (Join-Path $PSScriptRoot "verify-limit-rings.ps1")
