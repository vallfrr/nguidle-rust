//! NGUIdleRustEngine: sets the game up on the first launch, then plays it.

use ngu_prep::dirs::{self, Os};
use std::path::PathBuf;

fn data_root() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default();
    dirs::data_root(Os::current(), &home, |name| std::env::var(name).ok())
}

fn main() {
    let root = std::env::var_os("NGU_APP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(data_root);
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
