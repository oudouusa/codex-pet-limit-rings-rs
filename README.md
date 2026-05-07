# codex-pet-limit-rings-rs

Codex pets are tiny ambient companions for the work happening in Codex. This project adds one more layer to that idea: your pet can quietly show how much Codex capacity you have left, without turning the app into a dashboard.

The experience is a small native companion app for macOS and Windows. It watches where the Codex pet is, draws two polished rings around it, and keeps those rings attached to the pet as it moves. It does not patch Codex, change pet art, or modify the Codex app bundle.

It works with whatever Codex pet you like. Built-in pet, custom pet, tiny dog, robot, weather daemon, or anything else: the app does not care. It only follows the pet window that Codex is already showing.

![Codex Pet Limit Rings around a Codex pet](docs/assets/codex-pet-limit-rings-screenshot.png)

## What You See

The rings are designed to be glanceable:

- The outer ring shows the short-window limit remaining.
- The inner ring shows the weekly limit remaining.
- Color moves from calm green/blue to amber and red as capacity gets low.
- Hovering over the pet or rings shows the exact percentages at the current ring endpoints.
- A small menu-bar icon lets you hide the rings, refresh data, or quit.

When the Codex pet is closed, the rings disappear. When the pet comes back, they come back too. On multi-display setups, the rings stay with the pet instead of jumping to whichever screen is focused.

Because the rings are drawn in a separate transparent overlay, they do not need pet-specific sprites, masks, metadata, or configuration. Change pets in Codex and the rings follow the new one automatically.

## Why It Works This Way

The important design choice is the companion boundary. A menu item inside Codex itself would mean patching Electron app files and redoing that patch after app updates. That is brittle and hard to open source.

`codex-pet-limit-rings` stays outside the Codex app. It reads local Codex state, asks ChatGPT for live usage data using the local Codex/ChatGPT token, and renders its own transparent always-on-top window around the pet. The result is reversible, inspectable, and easy for another Codex agent to install or modify.

## Quick Start

### Windows / Rust

The Windows companion app lives under `tools/rust/` and uses the same
unpatched companion boundary. It tracks the live pet window, follows dragging
and momentum movement, handles mixed-DPI displays, and keeps the rings attached
to the pet.

Windows source installs require Rust/Cargo.

Run it from source:

```powershell
.\tools\run-limit-rings.ps1
```

Install it as a Startup shortcut:

```powershell
.\tools\install-limit-rings.ps1
```

Verify an installed copy:

```powershell
.\tools\verify-limit-rings.ps1
```

See `docs/windows-limit-rings.md` for details.

### macOS

Install the rings as a login item:

```bash
tools/install-limit-rings.sh
```

You should see a small rings icon in the macOS menu bar. Use that menu to toggle `Show Rings`, refresh the latest usage data, or quit.

Then use any Codex pet normally. No pet setup step is required.

Run a development build without installing the login item:

```bash
tools/run-limit-rings.sh
```

Uninstall everything the installer adds:

```bash
tools/uninstall-limit-rings.sh
```

## Give This Repo To Codex

This repository is structured so a Codex agent can pick it up from a GitHub link.

Ask the agent with one sentence:

```text
Install Codex Pet Limit Rings from https://github.com/oudouusa/codex-pet-limit-rings-rs for this computer, start it, and verify it is running.
```

The agent should read:

- `AGENTS.md` for the project contract.
- `skills/codex-pet-limit-rings/SKILL.md` for the install, debug, and validation workflow.
- `docs/limit-rings.md` for the data and rendering model.

To install the bundled skill into local Codex:

```bash
tools/install-codex-skill.sh
```

On Windows:

```powershell
.\tools\install-codex-skill.ps1
```

## Data And Privacy

The app reads only local Codex files and one ChatGPT usage endpoint:

- `~/.codex/.codex-global-state.json` or `%USERPROFILE%\.codex\.codex-global-state.json` tells it whether the pet is open and where it is.
- `~/.codex/auth.json` or `%USERPROFILE%\.codex\auth.json` provides the local bearer token used to read live usage from ChatGPT.
- `~/.codex/logs_2.sqlite` or `%USERPROFILE%\.codex\logs_2.sqlite` is used as a cached fallback if live usage is unavailable.

It does not require an OpenAI API key. It does not send pet images, screenshots, prompts, or repo contents anywhere.

## Project Shape

```text
tools/
  codex-pet-limit-rings.swift      native macOS companion app
  rust/                            native Windows companion app
  install-limit-rings.sh           build, install, and start at login
  install-limit-rings.ps1          build, install, and start at Windows login
  uninstall-limit-rings.sh         remove the app and login item
  uninstall-limit-rings.ps1        remove the Windows app and Startup shortcut
  run-limit-rings.sh               development launch
  run-limit-rings.ps1              Windows development launch
  verify-limit-rings.ps1           Windows install verification
  build-limit-rings.sh             app bundle builder
  install-codex-skill.sh           copy the bundled skill into ~/.codex/skills
  install-codex-skill.ps1          copy the bundled skill into %USERPROFILE%\.codex\skills

skills/codex-pet-limit-rings/
  SKILL.md                         Codex-agent workflow for this project

docs/
  limit-rings.md                   implementation contract and data flow
  windows-limit-rings.md           Windows companion app notes

experiments/weather-pets/
  earlier weather-pet renderer     kept as a separate experiment
```

## Development

Build the app:

```bash
tools/build-limit-rings.sh
```

Render a static preview PNG:

```bash
swiftc tools/codex-pet-limit-rings.swift -o tmp/codex-pet-limit-rings -framework AppKit -lsqlite3
tmp/codex-pet-limit-rings --preview tmp/limit-rings-preview.png --size 164
```

Build the Windows app:

```powershell
cargo build --manifest-path .\tools\rust\Cargo.toml --release
cargo run --manifest-path .\tools\rust\Cargo.toml -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

Validate the shell scripts:

```bash
bash -n tools/*.sh
```

## Experiments

The original exploration included a Python renderer for weather-mutated Codex pets. That work now lives under `experiments/weather-pets/` so the public repo can stay focused on limit rings while preserving the larger idea: Codex pets can become ambient interfaces for state, context, and mood.

## License

MIT. See `LICENSE`.

## Acknowledgements

This repository is derived from and inspired by the MIT-licensed
[`petergpt/codex-pet-limit-rings`](https://github.com/petergpt/codex-pet-limit-rings)
project. Portions of the documentation, repository structure, and companion-app
design were adapted from that project. See `NOTICE.md`.
