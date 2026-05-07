---
name: codex-pet-limit-rings
description: Install, run, customize, package, verify, or debug the Codex Pet Limit Rings companion app for Codex pets on macOS or Windows. Use when the user asks for Codex pet usage-limit rings, a menu-bar or notification-area toggle, launch-at-login or Startup packaging, live/cached Codex limit visualization, one-prompt Codex installation from this repository, or open-source distribution of the rings overlay.
---

# Codex Pet Limit Rings

## Core Rule

Keep the Codex desktop app unpatched by default. Ship and modify the rings as a companion app that reads local Codex state and exposes its own menu-bar icon on macOS or notification-area icon on Windows. Only discuss direct Codex app menu patching as a brittle optional route, because it requires `app.asar` patching, Electron integrity updates, and re-signing after Codex updates.

The rings are pet-agnostic. Do not add pet-specific setup unless a user explicitly asks for a custom visual treatment; by default the overlay follows whatever Codex pet is currently active.

## Locate The Project

If this skill is bundled in the repository, the project root is two directories above this `SKILL.md`. Otherwise find or ask for a checkout containing:

```text
tools/codex-pet-limit-rings.swift
tools/install-limit-rings.sh
tools/install-limit-rings.ps1
tools/run-limit-rings.sh
tools/run-limit-rings.ps1
```

If the user provides the GitHub repository URL and no checkout exists, clone or open that repository first, then use the checkout as the working directory. Read `AGENTS.md` first if it exists.

## Install

Choose the command for the current OS. On Windows, use PowerShell. On macOS, use Bash.

Windows install and verify:

```powershell
.\tools\install-limit-rings.ps1
```

macOS install and verify:

```bash
tools/install-limit-rings.sh
```

If install succeeds, report where the app was installed and whether the process is running. If the rings are offset, tell the user to place the cursor on the pet center and press `Ctrl+Alt+R` on Windows, or use the app menu on macOS.

## Run Without Installing

Windows:

```powershell
.\tools\run-limit-rings.ps1
```

macOS:

```bash
tools/run-limit-rings.sh
```

## Verify

Windows:

```powershell
.\tools\verify-limit-rings.ps1
```

macOS:

```bash
pgrep -fl CodexPetLimitRings
launchctl print "gui/$(id -u)/com.codex-pet.limit-rings" >/dev/null
```

## Uninstall

Windows:

```powershell
.\tools\uninstall-limit-rings.ps1
```

macOS:

```bash
tools/uninstall-limit-rings.sh
```

## Install This Skill Locally

Windows:

```powershell
.\tools\install-codex-skill.ps1
```

macOS:

```bash
tools/install-codex-skill.sh
```

## Data Contract

The rings read:

- `~/.codex/auth.json` or `%USERPROFILE%\.codex\auth.json` for a local ChatGPT access token, then `https://chatgpt.com/backend-api/wham/usage` for live usage data.
- `~/.codex/.codex-global-state.json` or `%USERPROFILE%\.codex\.codex-global-state.json` for `electron-avatar-overlay-open` and `electron-avatar-overlay-bounds`.
- `~/.codex/logs_2.sqlite` or `%USERPROFILE%\.codex\logs_2.sqlite` for fallback to the newest `codex.rate_limits` event when live usage fails.

The outer ring is the short-window remaining percentage. The inner ring is the weekly remaining percentage. The menu summary should say `Live` when direct usage succeeds and `Cached` when the local log fallback is active.

## Editing Workflow

For macOS behavior or visuals, edit `tools/codex-pet-limit-rings.swift`.

For Windows behavior or visuals, edit `tools/rust/`.

Keep packaging scripts in `tools/` and update `docs/limit-rings.md` or `docs/windows-limit-rings.md` when the user-facing contract changes.

Validate the relevant platform:

Windows:

```powershell
cargo build --manifest-path .\tools\rust\Cargo.toml --release
cargo run --manifest-path .\tools\rust\Cargo.toml -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

macOS:

```bash
bash -n tools/*.sh
swiftc tools/codex-pet-limit-rings.swift -o tmp/codex-pet-limit-rings -framework AppKit -lsqlite3
tmp/codex-pet-limit-rings --preview tmp/limit-rings-preview.png --size 164
```

## Open-Source Hygiene

Keep the app privacy-preserving, source-buildable, and uninstallable. Do not commit local `tmp/` builds, logs, derived pet spritesheets, or user-specific Codex data. Preserve the MIT license, keep `NOTICE.md` attribution when publishing a fork or derivative, and document any new local files or permissions in `docs/limit-rings.md` or `docs/windows-limit-rings.md`. For release prep, follow `docs/release-checklist.md`.
