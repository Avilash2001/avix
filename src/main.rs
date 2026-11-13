#![windows_subsystem = "windows"]
//! Avix - real-time Windows performance optimizer & debloater.
//!
//! One binary, two roles:
//!   * `avix.exe --tray`  - the background engine (tray icon, auto-boost, no GPU/UI).
//!   * `avix.exe --ui`    - the dashboard, spawned on demand and fully exited when closed.
//!   * `avix.exe`         - starts the engine if needed and opens the dashboard.
//!   * `avix.exe --restore` - emergency restore of anything a crashed boost left behind.
//!   * `avix.exe --diag`    - read-only self-test written to the data folder.
//!   * `--paused` starts the engine with auto-boost off; `--no-elevate` skips the UAC prompt.

mod autostart;
mod config;
mod diag;
mod engine;
mod health;
mod icon;
mod ipc;
mod logging;
#[cfg(debug_assertions)]
mod selftest;
mod sys;
mod tweaks;
mod update;
mod ui;

pub const APP_NAME: &str = "Avix";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a.eq_ignore_ascii_case(flag));

    if has("--diag") {
        // Read-only self-test; written next to the logs.
        let report = diag::run();
        let path = sys::data_dir().join("avix-diag.txt");
        let _ = std::fs::write(&path, &report);
        return;
    }

    #[cfg(debug_assertions)]
    if has("--selftest") {
        // Admin integration test (debug builds only) - see src/selftest.rs.
        let report = selftest::run();
        let _ = std::fs::write(sys::data_dir().join("avix-selftest.txt"), &report);
        return;
    }

    let role = if has("--ui") { "ui" } else { "engine" };
    logging::init(role);
    std::panic::set_hook(Box::new(|info| {
        logging::log(&format!("PANIC: {info}"));
    }));

    if !has("--no-elevate") && !sys::elevation::is_elevated() {
        if sys::elevation::relaunch_elevated(&args) {
            return;
        }
        logging::log("Elevation declined - running with limited features");
    }

    if has("--ui") {
        ui::run();
        return;
    }
    if has("--restore") {
        engine::journal::restore_from_disk();
        return;
    }

    let open_ui = !has("--tray");
    match engine::SingleInstance::acquire() {
        Some(guard) => engine::run(guard, open_ui, has("--paused")),
        None => {
            // An engine already runs in this session: just surface the dashboard.
            if open_ui && !ipc::Client::send_once(&ipc::Request::OpenUi) {
                engine::spawn_ui();
            }
        }
    }
}
