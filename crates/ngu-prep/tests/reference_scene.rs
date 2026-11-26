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

#[test]
#[ignore = "needs local game data"]
fn scripts_match_the_python_export_with_reference_layouts() {
    use ngu_prep::unity::typetree::Node;
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("NGU_REFERENCE_SCENE").expect("NGU_REFERENCE_SCENE")).unwrap(),
    )
    .unwrap();
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(std::env::var("NGU_ORACLE").expect("NGU_ORACLE")).unwrap())
            .unwrap();
    let files = scene::load_files(&data).unwrap();
    for f in &files { println!("file {} {}", f.name, f.objects.len()); }
    let infos = scene::scripts(&files).unwrap();
    let layout = |info: &scene::ScriptInfo| {
        let rows: Vec<(usize, String, String, u64)> = oracle[info.key()]
            .as_array()?
            .iter()
            .map(|r| {
                (
                    r[0].as_u64().unwrap() as usize,
                    r[1].as_str().unwrap().to_owned(),
                    r[2].as_str().unwrap().to_owned(),
                    r[3].as_u64().unwrap(),
                )
            })
            .collect();
        Some(std::rc::Rc::new(Node::from_flat(&rows)))
    };
    let mut ours = scene::plain_objects(&files).unwrap();
    ours.extend(scene::behaviours(&files, &infos, &layout).unwrap());
    scene::fit_card_labels(&mut ours);
    let mut checked = 0;
    for (key, record) in &ours {
        if record["t"] == "MonoBehaviour" {
            assert_eq!(record, &reference["objects"][key], "object {key} differs");
            checked += 1;
        }
    }
    for (key, record) in reference["objects"].as_object().unwrap() {
        if record["t"] == "MonoBehaviour" && !ours.contains_key(key) {
            println!("missing {key}");
            for f in &files { for o in f.objects.iter().filter(|o| format!("{}:{}", f.name, o.path_id) == *key) { println!("class {}", o.class_id); } }
        }
    }
    println!("{checked} scripts identical");
    assert_eq!(checked, 9630);
}
