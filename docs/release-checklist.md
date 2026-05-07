# Release Checklist

Use this before publishing a fork or derivative repository.

## Repository

- Keep `LICENSE` and `NOTICE.md`.
- Keep README install URLs pointed at the published repository URL.
- Do not commit `tmp/`, `target/`, local screenshots, Codex logs, auth files, or generated pet assets.
- Commit `tools/rust/Cargo.lock` with the Windows source build.

## Windows / Rust

```powershell
cargo fmt --manifest-path .\tools\rust\Cargo.toml --check
cargo build --manifest-path .\tools\rust\Cargo.toml --release
cargo run --manifest-path .\tools\rust\Cargo.toml -- --preview .\tmp\limit-rings-windows-preview.png --size 220
.\tools\install-limit-rings.ps1
.\tools\verify-limit-rings.ps1
```

## One-Prompt Install

After publishing, test this prompt in a fresh Codex session:

```text
Install Codex Pet Limit Rings from https://github.com/oudouusa/codex-pet-limit-rings-rs for this computer, start it, and verify it is running.
```

## License

The repository is MIT licensed. If code or documentation is copied from another
MIT project, preserve its license notice and keep attribution in `NOTICE.md` or
the README.
