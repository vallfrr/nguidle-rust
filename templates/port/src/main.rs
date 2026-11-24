//! Command line of the stand-in port: opens the real game, runs a few virtual frames and prints what was drawn.
//! PORT: this becomes your application's entry point (window, event loop, real clock).

use clr::session::{Session, SessionConfig};
use __PORT_LIB__::display::Display;
use __PORT_LIB__::input::Scripted;
use __PORT_LIB__::run_frame;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut data, mut save, mut frames) = (None, None, 120u32);
    while let Some(flag) = args.next() {
        match (flag.as_str(), args.next()) {
            ("--data", Some(value)) => data = Some(value),
            ("--save", Some(value)) => save = Some(value),
            ("--frames", Some(value)) => frames = value.parse().unwrap_or(frames),
            _ => {
                return eprintln!(
                    "usage: {} --data <prepared folder> --save <profile folder> [--frames N]",
                    env!("CARGO_PKG_NAME")
                )
            }
        }
    }
    let (Some(data), Some(save)) = (data, save) else {
        return eprintln!("--data and --save are required: see docs/PORTING.md in the engine repository, \"Prepare the data\".");
    };
    let mut session = match Session::open(SessionConfig::new(data, save, 960, 600)) {
        Ok(session) => session,
        Err(error) => return eprintln!("{error}"),
    };
    let (mut display, mut input) = (Display::default(), Scripted::new(Vec::new()));
    let mut last = Default::default();
    for _ in 0..frames {
        last = run_frame(&mut session, &mut input, &mut display, 1.0 / 60.0);
    }
    println!(
        "{frames} frames; last frame: {} quads; {} textures uploaded; fingerprint {:016x}",
        last.quads, display.uploads, display.fingerprint
    );
}
