//! The port's loop against the synthetic engine: no game files needed, so it runs everywhere (CI included).
//! Keep this passing while replacing the stand-ins; add a test for each platform behaviour you implement.

use clr::platform_api::{testing::ScriptedEngine, Action, Pointer};
use __PORT_LIB__::display::Display;
use __PORT_LIB__::input::{Event, Scripted};
use __PORT_LIB__::run_frame;

#[test]
fn input_time_drawing_text_and_sound_all_reach_the_engine() {
    let at = |down| {
        Event::Pointer(Pointer {
            x: 40.0,
            y: 30.0,
            down,
        })
    };
    let mut input = Scripted::new(vec![
        vec![at(true)],
        vec![at(false), Event::Action(Action::Activate)],
        vec![],
    ]);
    let (mut engine, mut display) = (ScriptedEngine::new(320, 200), Display::default());
    let first = run_frame(&mut engine, &mut input, &mut display, 1.0 / 30.0);
    assert_eq!(first.sounds, 1, "a press asks for a sound");
    let second = run_frame(&mut engine, &mut input, &mut display, 1.0 / 30.0);
    assert_eq!(
        second.text_requests, 1,
        "Activate opens a text field and the loop answers it"
    );
    assert_eq!(engine.submitted.len(), 1);
    run_frame(&mut engine, &mut input, &mut display, 1.0 / 30.0);
    assert_eq!(engine.steps, 3, "three 1/30 s frames are three logic steps");
    assert_eq!(engine.clicks, 1);
    assert_eq!(display.uploads, 1, "the checker texture is uploaded once");
    assert_eq!(
        engine.released_textures(),
        [2],
        "and released after the upload"
    );
    assert!(!display.frame.vertices.is_empty());
}
