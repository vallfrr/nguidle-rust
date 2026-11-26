//! Compares the Rust export with the one made by the Python tools.
//! Needs the game data: set NGU_GAME_DATA to its `NGUIdle_Data` folder,
//! NGU_REFERENCE_DIR to a folder exported by `tools/nguprep.py` and, for the
//! layout test, NGU_ORACLE to the type layouts written by the Python generator.
//! Then run `cargo test -p ngu-prep --test reference_scene -- --ignored`.

use ngu_prep::unity::scene;
use serde_json::Value;
use std::path::PathBuf;

#[test]
#[ignore = "needs local game data"]
fn plain_objects_match_the_python_export() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/scene.json",
            std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR")
        ))
        .unwrap(),
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
        &std::fs::read(format!(
            "{}/scene.json",
            std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR")
        ))
        .unwrap(),
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
        &std::fs::read(format!(
            "{}/scene.json",
            std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR")
        ))
        .unwrap(),
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

fn rgba(path: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let mut decoder =
        png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).unwrap();
    buffer.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgb => buffer
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        _ => buffer,
    };
    (info.width, info.height, pixels)
}

#[test]
#[ignore = "needs local game data"]
fn textures_fonts_and_texts_match_the_python_export() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference_dir =
        PathBuf::from(std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR"));
    let reference: Value =
        serde_json::from_slice(&std::fs::read(reference_dir.join("scene.json")).unwrap()).unwrap();
    let out = tempfile::tempdir().unwrap();
    let files = scene::load_files(&data).unwrap();
    let (records, errors) =
        ngu_prep::unity::assets::export(&files, &data, out.path(), true).unwrap();
    println!("errors: {errors:?}");
    let mut wrong = Vec::new();
    for (key, record) in &records {
        if record != &reference["objects"][key] {
            wrong.push(format!(
                "record {key}: {record} vs {}",
                reference["objects"][key]
            ));
        }
        let name = key.replace(':', "_");
        match record["t"].as_str().unwrap() {
            "Texture2D"
                if record["w"].as_i64().unwrap() > 0 && record["h"].as_i64().unwrap() > 0 =>
            {
                let (a, b) = (
                    out.path().join(format!("tex/{name}.png")),
                    reference_dir.join(format!("tex/{name}.png")),
                );
                if !a.exists() || rgba(&a) != rgba(&b) {
                    wrong.push(format!("pixels {key} format {}", record["fmt"]));
                }
            }
            "Font" => {
                if let Some(file) = record["file"].as_str() {
                    if std::fs::read(out.path().join("fonts").join(file)).unwrap()
                        != std::fs::read(reference_dir.join("fonts").join(file)).unwrap()
                    {
                        wrong.push(format!("font {key}"));
                    }
                }
            }
            "TextAsset" => {
                let file = format!("text/{name}.bin");
                if std::fs::read(out.path().join(&file)).unwrap()
                    != std::fs::read(reference_dir.join(&file)).unwrap()
                {
                    wrong.push(format!("text {key}"));
                }
            }
            _ => {}
        }
    }
    for line in wrong.iter().take(30) {
        println!("{line}");
    }
    println!("{} records, {} differences", records.len(), wrong.len());
    assert!(wrong.is_empty() && errors.is_empty());
}

#[test]
#[ignore = "needs local game data"]
fn archive_matches_the_python_archive() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference_dir =
        PathBuf::from(std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR"));
    let expected =
        std::fs::read(std::env::var("NGU_REFERENCE_PAK").expect("NGU_REFERENCE_PAK")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let pak = out.path().join("ngu.pak");
    ngu_prep::pack::pack(
        &reference_dir,
        &data.join("Managed/Assembly-CSharp.dll"),
        &pak,
    )
    .unwrap();
    assert!(std::fs::read(&pak).unwrap() == expected, "archives differ");
}

#[test]
#[ignore = "needs local game data"]
fn both_jobs_prepare_the_real_game() {
    let game = PathBuf::from(std::env::var("NGU_GAME_FOLDER").expect("NGU_GAME_FOLDER"));
    let out = tempfile::tempdir().unwrap();
    let vita =
        ngu_prep::prepare::prepare_vita(&game, &out.path().join("vita"), &mut |_| {}).unwrap();
    assert!(out.path().join("vita/nguidle/ngu.pak").is_file());
    assert!(vita.count > 3000, "{}", vita.count);
    let app = out.path().join("app");
    ngu_prep::prepare::prepare_pc(&game, &app, &mut |_| {}).unwrap();
    for file in [
        "game/NGUIdle_Data/Managed/Assembly-CSharp.dll",
        "data/scene.json",
        "data/tags.json",
        "data/Assembly-CSharp.dll",
    ] {
        assert!(app.join(file).is_file(), "{file}");
    }
    // A second run replaces the data and keeps the copy and the saves.
    std::fs::create_dir_all(app.join("saves")).unwrap();
    std::fs::write(app.join("saves/keep.txt"), b"x").unwrap();
    ngu_prep::prepare::prepare_pc(&game, &app, &mut |_| {}).unwrap();
    assert!(app.join("saves/keep.txt").is_file());
}

#[test]
#[ignore = "needs local game data"]
fn sounds_match_the_python_export() {
    let game = PathBuf::from(std::env::var("NGU_GAME_FOLDER").expect("NGU_GAME_FOLDER"));
    let reference_dir =
        PathBuf::from(std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR"));
    let player = std::fs::read(game.join("UnityPlayer.dll")).unwrap();
    let resource = std::fs::read(game.join("NGUIdle_Data/resources.resource")).unwrap();
    for (name, start, size) in [("Fart", 0, 61024), ("LongFart", 61024, 193952)] {
        let ours =
            ngu_prep::unity::audio::bank_to_wav(&resource[start..start + size], &player).unwrap();
        let expected = std::fs::read(reference_dir.join(format!("audio/{name}.wav"))).unwrap();
        assert_eq!(ours.len(), expected.len(), "{name}: length");
        let samples = |bytes: &[u8]| {
            bytes[44..]
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>()
        };
        let worst = samples(&ours)
            .iter()
            .zip(samples(&expected))
            .map(|(a, b)| (i32::from(*a) - i32::from(b)).abs())
            .max()
            .unwrap();
        println!(
            "{name}: {} bytes, largest sample difference {worst}",
            ours.len()
        );
        assert!(worst <= 2, "{name}: difference {worst}");
    }
}

#[test]
#[ignore = "needs local game data"]
fn full_export_matches_the_python_export() {
    let data = PathBuf::from(std::env::var("NGU_GAME_DATA").expect("NGU_GAME_DATA"));
    let reference_dir =
        PathBuf::from(std::env::var("NGU_REFERENCE_DIR").expect("NGU_REFERENCE_DIR"));
    let reference: Value =
        serde_json::from_slice(&std::fs::read(reference_dir.join("scene.json")).unwrap()).unwrap();
    let out = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let summary = ngu_prep::unity::export::export(&data, out.path()).unwrap();
    println!(
        "exported {} objects, {} resources in {:?}",
        summary.objects,
        summary.resources,
        started.elapsed()
    );
    assert!(summary.errors.is_empty(), "{:?}", summary.errors);
    let ours: Value =
        serde_json::from_slice(&std::fs::read(out.path().join("scene.json")).unwrap()).unwrap();
    assert_eq!(ours["scripts"], reference["scripts"], "scripts differ");
    assert_eq!(
        ours["resources"], reference["resources"],
        "resources differ"
    );
    for (key, record) in reference["objects"].as_object().unwrap() {
        assert_eq!(&ours["objects"][key], record, "object {key} differs");
    }
    assert_eq!(
        ours["objects"].as_object().unwrap().len(),
        reference["objects"].as_object().unwrap().len()
    );
    let tags = |dir: &std::path::Path| {
        serde_json::from_slice::<Value>(&std::fs::read(dir.join("tags.json")).unwrap()).unwrap()
    };
    assert_eq!(tags(out.path()), tags(&reference_dir));
    for name in ["Fart", "LongFart"] {
        let file = format!("audio/{name}.wav");
        let (a, b) = (
            std::fs::read(out.path().join(&file)).unwrap(),
            std::fs::read(reference_dir.join(&file)).unwrap(),
        );
        assert_eq!(a.len(), b.len(), "{name}");
    }
}
