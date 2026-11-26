//! Writes a picture of each setup screen: `cargo run -p ngu-app --example preview -- OUTPUT_FOLDER`.

use ngu_app::art::Art;
use ngu_app::browse::Browser;
use ngu_app::setup::{Model, Screen};
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let art = Art::load();
    let mut model = Model::new(
        out.join("app"),
        "A game file is missing: Managed/Assembly-CSharp.dll.".into(),
    );
    let found = vec![PathBuf::from("C:/Games/Steam/steamapps/common/NGU IDLE")];
    let screens: Vec<(&str, Screen)> = vec![
        ("intro", Screen::Intro),
        ("found", Screen::Found(found.clone())),
        ("found-none", Screen::Found(vec![])),
        ("found-many", Screen::Found(vec![found[0].clone(), PathBuf::from("D:/Library/steamapps/common/NGU IDLE")])),
        ("browse", Screen::Browse(Browser::open(&out), Some("This folder is not NGU Idle. Open the folder that contains NGU IDLE.exe.".into()))),
        ("working", Screen::Working),
        ("failed", Screen::Failed("This is not the supported NGU Idle build (1.260). Wrong SHA-256: Managed/Assembly-CSharp.dll".into())),
        ("done", Screen::Done),
    ];
    for (name, screen) in screens {
        model.screen = screen;
        let picture = ngu_app::preview::screen(&model, &art, 0.3);
        let file = std::fs::File::create(out.join(format!("{name}.png"))).unwrap();
        let mut encoder = png::Encoder::new(file, picture.width, picture.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&picture.rgba)
            .unwrap();
    }
}
