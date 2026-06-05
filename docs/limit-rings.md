# Codex Pet Limit Rings

Codex Pet Limit Rings is a Windows-only native Rust companion app for Codex pets. It does not patch Codex, replace pet art, or modify the Codex app bundle. It follows the current pet with a transparent always-on-top Win32 overlay and exposes its own notification-area icon.

The rings are pet-agnostic. They work with any pet Codex displays because the app tracks the pet window bounds rather than reading, editing, or understanding the pet artwork.

## Experience Contract

- A rings icon appears in the Windows notification area.
- `Show Rings` toggles the overlay without quitting the app.
- `Show When Pet Is Missing` displays a bottom-right fallback preview when Codex has not written pet bounds yet.
- `Refresh Now` rereads usage and pet-position state.
- Hovering over the ring or pet shows exact remaining percentages at the arc endpoints.
- Dragging the pet makes the rings follow the gesture while Codex persists the new position.
- Closing the Codex pet hides the rings unless fallback preview is enabled.
- Multi-display positioning uses the live pet overlay window and monitor DPI, so the rings stay attached during mixed-DPI movement.
- Switching to another Codex pet requires no extra setup.

## Data Flow

The app reads live usage first, then local files as support or fallback:

- `https://chatgpt.com/backend-api/wham/usage`: live usage endpoint, called with the local ChatGPT access token.
- `%USERPROFILE%\.codex\auth.json`: local ChatGPT auth token used for the live usage call.
- `%USERPROFILE%\.codex\.codex-global-state.json`: current pet state, using `electron-avatar-overlay-open` and `electron-avatar-overlay-bounds.mascot`.
- `%USERPROFILE%\.codex\logs_2.sqlite` or `logs_1.sqlite`: fallback source using the newest `codex.rate_limits` event when the live usage call fails.

No OpenAI API key is required. The menu summary says `Live` when the direct usage read succeeds and `Cached` when it is showing the local event-log fallback.

## Rendering Model

- Outer ring: short-window remaining percentage.
- Inner ring: weekly remaining percentage.
- Ring colors are derived from remaining capacity: green/blue for healthy, amber for low, red for critical.
- Exact percentages are shown only on hover to keep the pet feeling ambient rather than dashboard-like.
- The overlay is a layered window with per-pixel alpha so antialiased ring edges stay smooth on light backgrounds.

## Install Contract

`tools/install-limit-rings.ps1` builds the Rust app and publishes:

```text
%LOCALAPPDATA%\CodexPetLimitRings\CodexPetLimitRings.exe
```

It also creates a Startup shortcut:

```text
%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\CodexPetLimitRings.lnk
```

`tools/uninstall-limit-rings.ps1` stops the process, removes the Startup shortcut, and removes `%LOCALAPPDATA%\CodexPetLimitRings`.

## Development

Build and run the app from the repository:

```powershell
.\tools\run-limit-rings.ps1
```

Render a static preview:

```powershell
cargo run -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

## Codex Skill

The repository includes a skill at `skills/codex-pet-limit-rings/`. Copy that folder into `%USERPROFILE%\.codex\skills\` or run `tools\install-codex-skill.ps1` to make Codex auto-discover the workflow in future sessions.

The skill intentionally points agents at the companion-app boundary and validation commands. It should not encourage app-bundle patching as the default path.
