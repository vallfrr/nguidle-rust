//! Developer tool: runs the preparation jobs from the command line.
//!
//! `ngu-prepare pc GAME_FOLDER APP_FOLDER`, `ngu-prepare vita GAME_FOLDER OUTPUT_FOLDER`,
//! `ngu-prepare export GAME_FOLDER OUTPUT_FOLDER` (prepared folder for ports and developers),
//! `ngu-prepare web GAME_DATA OUTPUT_FOLDER` (web build cache) and `ngu-prepare verify GAME_FOLDER`.

use ngu_prep::prepare::{export_data, prepare_pc, prepare_vita, prepare_web, verified_game};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [job, game] = args.as_slice() {
        if job == "verify" {
            return match verified_game(Path::new(game)) {
                Ok(_) => {
                    println!("Steam Windows 1.260 files are intact.");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            };
        }
    }
    let [job, game, output] = args.as_slice() else {
        eprintln!("Usage: ngu-prepare pc|vita|export|web GAME_FOLDER OUTPUT_FOLDER, or ngu-prepare verify GAME_FOLDER");
        return ExitCode::from(2);
    };
    let mut say = |text: &str| println!("{text}");
    let result = match job.as_str() {
        "pc" => prepare_pc(Path::new(game), Path::new(output), &mut say),
        "vita" => prepare_vita(Path::new(game), Path::new(output), &mut say),
        "export" => export_data(Path::new(game), Path::new(output), &mut say),
        "web" => prepare_web(Path::new(game), Path::new(output), &mut say),
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
