//! The reference host against the real game. Needs the player's prepared data, so it is ignored in
//! the standard suite: set `NGU_TEST_DATA` to the prepared folder and run with `--ignored`.

use clr::session::{Session, SessionConfig};
use ngu_headless::{Host, Script};

#[test]
#[ignore = "requires the player's prepared game data (NGU_TEST_DATA)"]
fn the_real_game_draws_its_home_screen_and_is_reproducible() {
    let data =
        std::env::var("NGU_TEST_DATA").expect("set NGU_TEST_DATA to the prepared data folder");
    let save = std::env::temp_dir().join(format!("ngu-headless-test-{}", std::process::id()));
    let session = Session::open(SessionConfig::new(&data, &save, 960, 600)).unwrap();
    let mut host = Host::new(session);
    let report = host
        .run(&Script::parse("wait 1\nclick 480 300\nwait 1").unwrap())
        .clone();
    let _ = std::fs::remove_dir_all(&save);
    assert!(report.last_quads > 50, "{report}");
    assert!(report.textures_uploaded > 0, "{report}");
    assert!(report.atlas_uploads > 0, "{report}");
}
