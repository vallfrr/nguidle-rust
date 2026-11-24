//! The reference host against the synthetic engine: no game files, runs in CI.

use ngu_headless::{Host, Script};
use platform_api::testing::ScriptedEngine;

fn run(script: &str) -> ngu_headless::Report {
    let mut host = Host::new(ScriptedEngine::new(320, 200));
    host.run(&Script::parse(script).unwrap()).clone()
}

#[test]
fn a_click_is_delivered_drawn_uploaded_and_heard() {
    let report = run("wait 0.2\nclick 40 30\naction activate\ntext Alice\nwait 0.1\ndraw");
    assert!(report.frames >= 10, "{report}");
    assert_eq!(report.sounds, 1, "{report}");
    assert_eq!(report.textures_uploaded, 1, "the checker texture: {report}");
    assert_eq!(report.text_requests, 1, "{report}");
    assert_eq!(report.atlas_uploads, 1, "{report}");
    assert_eq!(
        report.last_quads, 3,
        "background, one square, cursor: {report}"
    );
}

#[test]
fn a_run_is_reproducible_and_sensitive_to_input() {
    let first = run("wait 0.3\nclick 10 10\ndraw");
    assert_eq!(first, run("wait 0.3\nclick 10 10\ndraw"));
    assert_ne!(first.hash, run("wait 0.3\nclick 12 10\ndraw").hash);
}

#[test]
fn text_can_be_cancelled() {
    let mut host = Host::new(ScriptedEngine::new(320, 200));
    host.run(&Script::parse("action activate\ntext -\ndraw").unwrap());
    assert_eq!(host.engine.submitted, [None]);
}

#[test]
fn script_errors_say_what_to_fix() {
    let error = Script::parse("wait 1\njump 3").unwrap_err();
    assert!(
        error.contains("line 2") && error.contains("unknown step `jump`"),
        "{error}"
    );
    assert!(Script::parse("click 1")
        .unwrap_err()
        .contains("two numbers"));
    assert!(Script::parse("action sideways")
        .unwrap_err()
        .contains("up, down"));
    assert_eq!(Script::parse("# only a comment\n\n").unwrap().0.len(), 0);
}
