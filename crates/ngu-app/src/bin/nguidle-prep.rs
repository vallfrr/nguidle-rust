//! nguidle-prep: writes the `nguidle` folder a PS Vita needs, from the player's own copy of the game.
// Release builds on Windows have no console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::io::Write;

fn main() {
    let log = std::env::temp_dir().join("nguidle-prep.log");
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            let _ = writeln!(file, "{info}");
        }
        eprintln!("{info}");
    }));
    ngu_app::setup::run_vita();
}
