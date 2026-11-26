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
    let found = vec![PathBuf::from("Games/Steam/steamapps/common/NGU IDLE")];
    let screens: Vec<(&str, Screen)> = vec![
        ("intro", Screen::Intro),
        ("found", Screen::Found(found.clone())),
        ("found-none", Screen::Found(vec![])),
        ("found-many", Screen::Found(vec![found[0].clone(), PathBuf::from("Library/steamapps/common/NGU IDLE")])),
        ("browse", Screen::Browse(Browser::open(&out), Some("This folder is not NGU Idle: its game files are missing.".into()))),
        ("working", Screen::Working),
        ("failed", Screen::Failed("This is not the supported NGU Idle build (1.260). Wrong SHA-256: Managed/Assembly-CSharp.dll".into())),
        ("done", Screen::Done),
    ];
    let mut vita = Model::vita();
    let vita_screens: Vec<(&str, Screen)> =
        vec![("vita-intro", Screen::Intro), ("vita-done", Screen::Done)];
    for (name, screen) in vita_screens {
        vita.screen = screen;
        vita.output = Some(PathBuf::from("Documents/nguidle"));
        write(&out, name, &ngu_app::preview::screen(&vita, &art, 0.3));
    }
    vita.choosing_output = true;
    vita.screen = Screen::Browse(Browser::open(&out), None);
    write(
        &out,
        "vita-output",
        &ngu_app::preview::screen(&vita, &art, 0.3),
    );
    for (name, screen) in screens {
        model.job.lock().unwrap().text = "Preparing the game data...".into();
        model.screen = screen;
        let picture = ngu_app::preview::screen(&model, &art, 0.3);
        write(&out, name, &picture);
    }
}

fn write(out: &std::path::Path, name: &str, picture: &ngu_app::art::Image) {
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
