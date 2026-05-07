$ErrorActionPreference = "Stop"

$InstallDir = Join-Path $env:LOCALAPPDATA "CodexPetLimitRings"
$Exe = Join-Path $InstallDir "CodexPetLimitRings.exe"
$Startup = Join-Path ([Environment]::GetFolderPath("Startup")) "CodexPetLimitRings.lnk"
$Process = Get-Process -Name "CodexPetLimitRings" -ErrorAction SilentlyContinue | Select-Object -First 1

$Checks = @(
    [PSCustomObject]@{ Name = "installed exe"; Ok = Test-Path -LiteralPath $Exe; Path = $Exe },
    [PSCustomObject]@{ Name = "startup shortcut"; Ok = Test-Path -LiteralPath $Startup; Path = $Startup },
    [PSCustomObject]@{ Name = "running process"; Ok = $null -ne $Process; Path = if ($Process) { "pid $($Process.Id)" } else { "" } }
)

$Checks | ForEach-Object {
    $Status = if ($_.Ok) { "ok" } else { "missing" }
    Write-Host "$Status`t$($_.Name)`t$($_.Path)"
}

if ($Checks.Ok -contains $false) {
    throw "Codex Pet Limit Rings verification failed."
}
