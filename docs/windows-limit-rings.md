# Windows Codex Pet Limit Rings

The Windows build follows the same boundary as the macOS version: it is a separate Rust companion app. It does not patch Codex or modify pet sprites.

## What It Does

- Reads live usage from `https://chatgpt.com/backend-api/wham/usage` using the local token in `%USERPROFILE%\.codex\auth.json`.
- Falls back to the newest `codex.rate_limits` event in `%USERPROFILE%\.codex\logs_2.sqlite`.
- Reads the Codex pet position from `%USERPROFILE%\.codex\.codex-global-state.json` when `electron-avatar-overlay-bounds.mascot` is available.
- Tracks the live Codex pet overlay window as the physical coordinate source, so the rings keep up during movement and moves between displays with different scale settings.
- Draws a transparent Win32 layered overlay around the current pet and places it directly behind the pet window.
- Uses a layered window with per-pixel alpha so antialiased ring edges remain smooth on light backgrounds.
- Uses the live overlay window's physical bounds plus Codex's saved `mascot` coordinates inside that window, avoiding saved absolute coordinates that can use a different coordinate scale.
- Ignores hover-only and status-panel overlay changes so Codex's pet controls and session updates do not leave the rings offset.
- Draws strong arcs for remaining capacity and softer same-hue arcs for the used portions without transparency-key blending artifacts on white backgrounds.
- Adds a Windows notification-area icon with show/hide, fallback preview, alignment nudges, refresh, and quit actions.

If Codex has not written pet bounds yet, the overlay hides by default. Use `--show-without-pet` or the tray menu item `Show When Pet Is Missing` to display a bottom-right preview ring.

If the ring is slightly offset from the pet, use the notification-area menu:

```text
Left/Right/Up/Down
```

For mixed-DPI or multi-window setups, place the mouse cursor on the pet center and press:

```text
Ctrl+Alt+R
```

The offset is saved in:

```text
%LOCALAPPDATA%\CodexPetLimitRings\offset-x.txt
%LOCALAPPDATA%\CodexPetLimitRings\offset-y.txt
```

You can also start with explicit offsets:

```powershell
cargo run --manifest-path .\tools\rust\Cargo.toml -- --offset-x 8 --offset-y -4
```

## Run From Source

Windows source builds require Rust/Cargo.

```powershell
.\tools\run-limit-rings.ps1
```

Preview a static PNG:

```powershell
cargo run --manifest-path .\tools\rust\Cargo.toml -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

## Install At Login

The installer builds the Rust app from source, copies the release executable to `%LOCALAPPDATA%\CodexPetLimitRings`, and creates a Startup shortcut.

```powershell
.\tools\install-limit-rings.ps1
```

This publishes the app to:

```text
%LOCALAPPDATA%\CodexPetLimitRings
```

and creates a Startup shortcut:

```text
%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\CodexPetLimitRings.lnk
```

Verify an installed copy:

```powershell
.\tools\verify-limit-rings.ps1
```

## Install With One Codex Prompt

Give Codex this sentence:

```text
Install Codex Pet Limit Rings from https://github.com/oudouusa/codex-pet-limit-rings-rs for this computer, start it, and verify it is running.
```

The repository includes `AGENTS.md` and
`skills/codex-pet-limit-rings/SKILL.md` so the agent can choose the Windows
installer, run the verification script, and report the result.

## Uninstall

```powershell
.\tools\uninstall-limit-rings.ps1
```
