//! `ngu-headless`: run the real game with no window and print what a port would have drawn.
//!
//! ```text
//! ngu-headless --data <prepared folder> --save <profile folder> [--size 960x600] [--script file]
//! ```
//! Without `--script` it waits one second of virtual time and draws the home screen.

use clr::session::{Session, SessionConfig};
use ngu_headless::{Host, Script};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ngu-headless: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut data = None;
    let mut save = None;
    let mut size = (960u32, 600u32);
    let mut script = String::from("wait 1\n");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |what: &str| args.next().ok_or_else(|| format!("{flag} needs {what}"));
        match flag.as_str() {
            "--data" => data = Some(value("a folder")?),
            "--save" => save = Some(value("a folder")?),
            "--size" => {
                let text = value("WIDTHxHEIGHT")?;
                size = text
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or("--size expects WIDTHxHEIGHT, for example 960x600")?;
            }
            "--script" => {
                let path = value("a file")?;
                script = std::fs::read_to_string(&path)
                    .map_err(|e| format!("cannot read {path}: {e}"))?;
            }
            other => {
                return Err(format!(
                    "unknown option {other}; see the header of src/main.rs"
                ))
            }
        }
    }
    let data =
        data.ok_or("--data <prepared folder> is required (docs/PORTING.md, \"Prepare the data\")")?;
    let save =
        save.ok_or("--save <profile folder> is required (use a throw-away folder for tests)")?;
    let script = Script::parse(&script)?;
    let session =
        Session::open(SessionConfig::new(data, save, size.0, size.1)).map_err(|e| e.to_string())?;
    let mut host = Host::new(session);
    println!("{}", host.run(&script));
    Ok(())
}
