$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Source = Join-Path $Root "skills\codex-pet-limit-rings"

if (-not (Test-Path -LiteralPath (Join-Path $Source "SKILL.md"))) {
    throw "Bundled skill was not found: $Source"
}

$CodexHome = $env:CODEX_HOME
if ([string]::IsNullOrWhiteSpace($CodexHome)) {
    $CodexHome = Join-Path $HOME ".codex"
}

$SkillsDir = Join-Path $CodexHome "skills"
$Destination = Join-Path $SkillsDir "codex-pet-limit-rings"

New-Item -ItemType Directory -Force -Path $SkillsDir | Out-Null
Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
Copy-Item -LiteralPath $Source -Destination $Destination -Recurse

Write-Host "Installed Codex skill at $Destination"
