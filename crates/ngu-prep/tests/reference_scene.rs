//! Compares the Rust export with a scene exported by the Python tool.
//! Needs the game data: set NGU_GAME_DATA and NGU_REFERENCE_SCENE, then run
//! `cargo test -p ngu-prep --test reference_scene -- --ignored`.

use ngu_prep::unity::scene;
use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "needs local game data"]
fn plain_objects_match_the_python_export() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("NGU_REFERENCE_SCENE").expect("NGU_REFERENCE_SCENE")).unwrap(),
    )
    .unwrap();
    let files = scene::load_files(&data).unwrap();
    let mut ours = scene::plain_objects(&files).unwrap();
    scene::park_card_preview(&mut ours);
    scene::hide_unavailable_online_ui(&mut ours);
    let mut checked = 0;
    for (key, record) in &ours {
        let expected = &reference["objects"][key];
        let (got, want) = (record, expected);
        assert_eq!(&got, &want, "object {key} differs");
        checked += 1;
    }
    println!("{checked} objects identical");
    assert!(checked > 10_000);
}
