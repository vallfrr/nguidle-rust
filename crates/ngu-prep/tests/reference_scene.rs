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
fn scripts_match_the_python_export() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("NGU_REFERENCE_SCENE").expect("NGU_REFERENCE_SCENE")).unwrap(),
    )
    .unwrap();
    let files = scene::load_files(&data).unwrap();
    for f in &files {
        println!("file {} {}", f.name, f.objects.len());
    }
    let infos = scene::scripts(&files).unwrap();
    let layouts = ngu_prep::unity::layout::Layouts::new(data.join("Managed"));
    let layout = |info: &scene::ScriptInfo| {
        let class = info.key().split_once('|').unwrap().1.to_owned();
        layouts
            .script(&info.assembly, &class)
            .ok()
            .map(std::rc::Rc::new)
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
            for f in &files {
                for o in f
                    .objects
                    .iter()
                    .filter(|o| format!("{}:{}", f.name, o.path_id) == *key)
                {
                    println!("class {}", o.class_id);
                }
            }
        }
    }
    println!("{checked} scripts identical");
    assert_eq!(checked, 9630);
}

fn first_difference(
    a: &ngu_prep::unity::typetree::Node,
    b: &ngu_prep::unity::typetree::Node,
    path: &str,
) -> Option<String> {
    let here = format!("{path}/{}", a.name);
    if (&a.ty, &a.name, a.flags) != (&b.ty, &b.name, b.flags) {
        return Some(format!(
            "{here}: ours {} {} {:#x}, expected {} {} {:#x}",
            a.ty, a.name, a.flags, b.ty, b.name, b.flags
        ));
    }
    if a.children.len() != b.children.len() {
        let names = |n: &ngu_prep::unity::typetree::Node| {
            n.children
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>()
        };
        return Some(format!(
            "{here}: ours {:?}, expected {:?}",
            names(a),
            names(b)
        ));
    }
    a.children
        .iter()
        .zip(&b.children)
        .find_map(|(x, y)| first_difference(x, y, &here))
}

#[test]
#[ignore = "needs local game data"]
fn generated_layouts_match_the_python_generator() {
    use ngu_prep::unity::{layout, typetree::Node};
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let oracle: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("NGU_ORACLE").expect("NGU_ORACLE")).unwrap(),
    )
    .unwrap();
    let reference: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("NGU_REFERENCE_SCENE").expect("NGU_REFERENCE_SCENE")).unwrap(),
    )
    .unwrap();
    let layouts = layout::Layouts::new(data.join("Managed"));
    let mut used = std::collections::BTreeSet::new();
    for record in reference["objects"].as_object().unwrap().values() {
        if record["t"] == "MonoBehaviour" {
            let s = &record["script"];
            let namespace = s["ns"].as_str().unwrap();
            let class = s["cls"].as_str().unwrap();
            let full = if namespace.is_empty() {
                class.to_owned()
            } else {
                format!("{namespace}.{class}")
            };
            used.insert((s["asm"].as_str().unwrap().to_owned(), full));
        }
    }
    let (mut good, mut bad) = (0, Vec::new());
    for (assembly, class) in &used {
        let rows: Vec<(usize, String, String, u64)> = oracle[format!("{assembly}|{class}")]
            .as_array()
            .unwrap()
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
        let expected = Node::from_flat(&rows);
        match layouts.script(assembly, class) {
            Ok(ours) => match first_difference(&ours, &expected, "") {
                None => good += 1,
                Some(d) => bad.push(format!("{class}: {d}")),
            },
            Err(e) => bad.push(format!("{class}: ERROR {e}")),
        }
    }
    for line in bad.iter().take(40) {
        println!("{line}");
    }
    println!("{good} layouts identical, {} different", bad.len());
    assert!(bad.is_empty());
}
