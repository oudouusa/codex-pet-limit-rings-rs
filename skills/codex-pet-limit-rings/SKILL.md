---
name: codex-pet-limit-rings
description: Install, run, customize, package, verify, or debug the Windows-only Rust Codex Pet Limit Rings companion app for Codex pets. Use when the user asks for Codex pet usage-limit rings, a Windows notification-area toggle, Startup packaging, live/cached Codex limit visualization, one-prompt Codex installation from this repository, or open-source distribution of the rings overlay.
---

# Codex Pet Limit Rings

## Core Rule

Keep the Codex desktop app unpatched. Ship and modify the rings as a Windows companion app that reads local Codex state and exposes its own notification-area icon. Only discuss direct Codex app menu patching as a brittle optional route, because it requires `app.asar` patching, Electron integrity updates, and re-signing after Codex updates.

The rings are pet-agnostic. Do not add pet-specific setup unless a user explicitly asks for a custom visual treatment; by default the overlay follows whatever Codex pet is currently active.

## Locate The Project

If this skill is bundled in the repository, the project root is two directories above this `SKILL.md`. Otherwise find or ask for a checkout containing:

```text
Cargo.toml
src/windows_app.rs
tools/install-limit-rings.ps1
tools/run-limit-rings.ps1
tools/verify-limit-rings.ps1
```

If the user provides the GitHub repository URL and no checkout exists, clone or open that repository first, then use the checkout as the working directory. Read `AGENTS.md` first if it exists.

## Install

Use PowerShell on Windows. Verify the user has Rust/Cargo and a working Windows Rust build environment before installing from source. The expected setup is documented in `docs/windows-limit-rings.md`: Windows 10/11, PowerShell, Rust stable, and the MSVC build tools or another working Windows linker/toolchain.

Install and verify:

```powershell
.\tools\install-limit-rings.ps1
```

If install succeeds, report where the app was installed and whether the process is running. If the rings are offset, tell the user to place the cursor on the pet center and press `Ctrl+Alt+R`, or use the notification-area menu nudge commands.

## Run Without Installing

```powershell
.\tools\run-limit-rings.ps1
```

## Verify

```powershell
.\tools\verify-limit-rings.ps1
```

## Uninstall

```powershell
.\tools\uninstall-limit-rings.ps1
```

## Install This Skill Locally

```powershell
.\tools\install-codex-skill.ps1
```

## Data Contract

The rings read:

- `%USERPROFILE%\.codex\auth.json` for a local ChatGPT access token, then `https://chatgpt.com/backend-api/wham/usage` for live usage data.
- `%USERPROFILE%\.codex\.codex-global-state.json` for `electron-avatar-overlay-open` and `electron-avatar-overlay-bounds`.
- `%USERPROFILE%\.codex\logs_2.sqlite` or `logs_1.sqlite` for fallback to the newest `codex.rate_limits` event when live usage fails.

The outer ring is the short-window remaining percentage. The inner ring is the weekly remaining percentage. The menu summary should say `Live` when direct usage succeeds and `Cached` when the local log fallback is active.

## Editing Workflow

For Windows behavior or visuals, edit `src/windows_app.rs`.

Keep packaging scripts in `tools/` and update `docs/limit-rings.md` or `docs/windows-limit-rings.md` when the user-facing contract changes.

Validate on Windows:

```powershell
cargo fmt --check
cargo build --release
cargo run -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

## Open-Source Hygiene

Keep the app privacy-preserving, source-buildable, and uninstallable. Do not commit local `tmp/` builds, `target/`, logs, derived pet spritesheets, bearer tokens, or user-specific Codex data. Preserve the MIT license, keep attribution in `NOTICE.md` and the README, and document any new local files or permissions in `docs/limit-rings.md` or `docs/windows-limit-rings.md`. For release prep, follow `docs/release-checklist.md`.
