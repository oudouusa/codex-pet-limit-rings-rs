$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$Manifest = Join-Path $Root "tools\rust\Cargo.toml"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo was not found. Install Rust before running Codex Pet Limit Rings from source."
}

cargo run --manifest-path $Manifest --release -- --show-without-pet @args
