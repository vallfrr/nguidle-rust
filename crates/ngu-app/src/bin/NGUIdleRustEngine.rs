//! NGUIdleRustEngine: sets the game up on the first launch, then plays it.
// Release builds on Windows have no console window; problems go to a log file instead.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use ngu_prep::dirs::{self, Os};
use std::io::Write;
use std::path::{Path, PathBuf};

fn data_root() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default();
    dirs::data_root(Os::current(), &home, |name| std::env::var(name).ok())
}

/// Writes any crash to `NGUIdleRustEngine.log` in the data folder, since there may be no console.
fn log_crashes(root: &Path) {
    let file = root.join("NGUIdleRustEngine.log");
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
        {
            let _ = writeln!(log, "{info}");
        }
        eprintln!("{info}");
    }));
}

fn main() {
    let root = std::env::var_os("NGU_APP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(data_root);
    log_crashes(&root);
    let forced = std::env::args().any(|a| a == "--setup");
    match ngu_prep::prepare::check_prepared(&root) {
        Ok(()) if !forced => ngu_app::play::run(&root),
        Ok(()) => ngu_app::setup::run(root, String::new()),
        Err(reason) => {
            // Nothing at all means a first launch: no explanation needed.
            let first = !root.join("game").exists() && !root.join("data").exists();
            ngu_app::setup::run(root, if first { String::new() } else { reason });
        }
    }
}
