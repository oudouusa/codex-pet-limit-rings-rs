# Codex Pet Limit Rings Agent Notes

## Goal

This repository packages `codex-pet-limit-rings-rs`: a Windows-only native Rust companion app that draws usage-limit rings around the current Codex pet without patching Codex.

## Primary Contract

- Keep the Codex app bundle unmodified.
- Treat `src/windows_app.rs` as the Windows app source.
- Treat `tools/install-limit-rings.ps1`, `tools/run-limit-rings.ps1`, `tools/verify-limit-rings.ps1`, and `tools/uninstall-limit-rings.ps1` as the supported install and runtime path.
- Treat `skills/codex-pet-limit-rings/SKILL.md` as the reusable Codex-agent workflow.
- Keep this repository Windows/Rust focused. Do not reintroduce macOS Swift code, shell installers, plist packaging, or weather-pet experiments unless the repository scope changes.
- Keep `LICENSE`, `NOTICE.md`, and `docs/release-checklist.md` current for public distribution.

## Done When

For Windows app changes, verify on Windows:

```powershell
cargo fmt --check
cargo build --release
cargo run -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

For packaged installs, also run:

```powershell
.\tools\install-limit-rings.ps1
.\tools\verify-limit-rings.ps1
```

Do not commit `tmp/`, `target/`, local logs, screenshots, user Codex state, bearer tokens, or generated private pet assets.

For public release prep, also read `docs/release-checklist.md`.
