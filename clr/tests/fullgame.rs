//! The full-game script on the player's prepared data. Not part of `just check` (needs the game):
//! `NGU_TEST_PREPARED=<prepared folder> cargo test --release -p clr --test fullgame -- --ignored --nocapture`.

use clr::fullgame::{report, run, Options, Status};
use clr::Vm;

#[test]
#[ignore = "requires the player's prepared game data (NGU_TEST_PREPARED)"]
fn the_whole_game_plays_without_a_ko() {
    let data = std::env::var("NGU_TEST_PREPARED").expect("set NGU_TEST_PREPARED to the prepared data folder");
    let work = std::env::temp_dir().join(format!("ngu-fullgame-test-{}", std::process::id()));
    let mut vm = Vm::new(&format!("{data}/Assembly-CSharp.dll"));
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = work.join("save").to_string_lossy().into_owned();
    std::fs::create_dir_all(&vm.eng.save_dir).unwrap();
    clr::unity_api::load_prefs(&mut vm);
    vm.load_scene(&data).expect("scene");
    vm.awake_all();
    for _ in 0..40 {
        vm.frame(1.0 / 30.0);
    }
    let checks = run(&mut vm, &Options { hours: 0.05, workdir: work });
    let (text, ok) = report(&checks);
    assert!(ok, "{text}");
    assert!(checks.iter().all(|c| c.status != Status::Skip), "the real game skips nothing:\n{text}");
}
