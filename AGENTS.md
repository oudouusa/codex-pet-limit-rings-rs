# Codex Pet Limit Rings Agent Notes

## Goal

This repository packages `codex-pet-limit-rings`: a native companion app that draws usage-limit rings around the current Codex pet without patching Codex. It supports macOS and Windows.

## Primary Contract

- Keep the Codex app bundle unmodified.
- Treat `tools/codex-pet-limit-rings.swift` as the macOS app source.
- Treat `tools/rust/` as the Windows app source.
- Treat `tools/install-limit-rings.sh` and `tools/uninstall-limit-rings.sh` as the macOS install/uninstall path.
- Treat `tools/install-limit-rings.ps1` and `tools/uninstall-limit-rings.ps1` as the Windows install/uninstall path.
- Treat `skills/codex-pet-limit-rings/SKILL.md` as the reusable Codex-agent workflow.
- Keep weather-pet code under `experiments/weather-pets/`; it is not the main package.
- Keep `LICENSE`, `NOTICE.md`, and `docs/release-checklist.md` current for public distribution.

## Done When

For app changes, verify:

```bash
bash -n tools/*.sh
swiftc tools/codex-pet-limit-rings.swift -o tmp/codex-pet-limit-rings -framework AppKit -lsqlite3
tmp/codex-pet-limit-rings --preview tmp/limit-rings-preview.png --size 164
```

For packaged installs, also run `tools/install-limit-rings.sh` and verify:

```bash
pgrep -fl CodexPetLimitRings
launchctl print "gui/$(id -u)/com.codex-pet.limit-rings" >/dev/null
```

Do not commit `tmp/`, local logs, screenshots, user Codex state, or generated private pet assets.
For public release prep, also read `docs/release-checklist.md`.

For Windows packaged installs, run:

```powershell
.\tools\install-limit-rings.ps1
.\tools\verify-limit-rings.ps1
```

For Windows app changes, verify:

```powershell
cargo build --manifest-path .\tools\rust\Cargo.toml --release
cargo run --manifest-path .\tools\rust\Cargo.toml -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```
