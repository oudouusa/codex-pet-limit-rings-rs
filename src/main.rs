#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod windows_app;

#[cfg(target_os = "windows")]
fn main() {
    windows_app::run();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "codex-pet-limit-rings-rs is Windows-only. Build and run it on Windows with `cargo run --release`."
    );
}
