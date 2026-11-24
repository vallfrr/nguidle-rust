//! __PORT_NAME__: an NGU Idle port for __PORT_PLATFORM__.
//!
//! Every place where a real platform needs its own code is marked `PORT:`. Everything else is the
//! contract with the engine and should stay as it is: see `docs/PORTING.md` in the engine repository.

pub mod audio;
pub mod display;
pub mod input;

use clr::platform_api::{Engine, STEP_SECONDS};
use display::Display;
use input::Source;

/// What one pass of the main loop did, for logging and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameSummary {
    /// Quads drawn.
    pub quads: usize,
    /// Sounds requested.
    pub sounds: usize,
    /// Text requests answered.
    pub text_requests: usize,
}

/// One iteration of the main loop. A real port calls this once per display frame with the real elapsed time.
///
/// The order matters: input first, then game time, then the draw, then everything the draw implies.
pub fn run_frame<E: Engine>(
    engine: &mut E,
    input: &mut dyn Source,
    display: &mut Display,
    elapsed: f32,
) -> FrameSummary {
    // 1. Input. PORT: translate the platform's events in `input::Source`.
    for event in input.poll() {
        input::deliver(engine, event);
    }
    // 2. Game time: the engine steps its logic in whole 1/30 s steps.
    engine.advance(elapsed.min(STEP_SECONDS * 8.0));
    // 3. Draw. PORT: `Display` receives the ordered quads; draw them with your graphics API.
    let quads = engine.draw(display);
    display.upload_missing(engine);
    // 4. Text fields. PORT: show your keyboard or text box; answer with `submit_text`.
    let mut text_requests = 0;
    if let Some(request) = engine.take_text_request() {
        text_requests += 1;
        engine.submit_text(
            input
                .edit_text(&request.text, request.max_len, request.numeric)
                .as_deref(),
        );
    }
    // 5. Sound. PORT: play the named clip (`audio::play`).
    let sounds = engine.take_sounds();
    for clip in &sounds {
        audio::play(clip);
    }
    FrameSummary {
        quads,
        sounds: sounds.len(),
        text_requests,
    }
}
