//! Developer tool: runs the preparation jobs from the command line.
//!
//! `ngu-prepare pc GAME_FOLDER APP_FOLDER` and `ngu-prepare vita GAME_FOLDER OUTPUT_FOLDER`.

use ngu_prep::prepare::{prepare_pc, prepare_vita};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [job, game, output] = args.as_slice() else {
        eprintln!("Usage: ngu-prepare pc|vita GAME_FOLDER OUTPUT_FOLDER");
        return ExitCode::from(2);
    };
    let mut say = |text: &str| println!("{text}");
    let result = match job.as_str() {
        "pc" => prepare_pc(Path::new(game), Path::new(output), &mut say),
        "vita" => prepare_vita(Path::new(game), Path::new(output), &mut say),
        _ => {
            eprintln!("Unknown job {job}.");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(report) => {
            for warning in &report.warnings {
                println!("Note: {warning}");
            }
            println!("Done ({} items).", report.count);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
