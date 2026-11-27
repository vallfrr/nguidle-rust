//! A host that was away (a hidden tab, a sleeping computer) gives the game its offline progress through `Session::advance`.
//! Not part of `just check` (needs the game): `NGU_TEST_PREPARED=<prepared folder> cargo test --release -p clr --test away_session -- --ignored`.

use clr::platform_api::Engine;
use clr::session::{Session, SessionConfig};

#[test]
#[ignore = "requires the player's prepared game data (NGU_TEST_PREPARED)"]
fn a_host_that_was_away_gets_offline_progress() {
    let data = std::env::var("NGU_TEST_PREPARED").expect("set NGU_TEST_PREPARED to the prepared data folder");
    let work = std::env::temp_dir().join(format!("ngu-away-test-{}", std::process::id()));
    let mut session = Session::open(SessionConfig::new(&data, work.join("save"), 960, 600)).expect("session");
    let start = 1_700_000_000u64;
    std::env::set_var("NGU_FIXED_NOW", start.to_string());
    let steps: Vec<String> = ["click,Create New Game", "click,Slot 1", "click,Create New Game", "w,30", "click,PLAY", "w,10"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    session.run_script(&steps);
    for _ in 0..10 {
        session.advance(1.0 / 60.0);
    }
    assert_eq!(session.vm().eng.resumed, 0, "nothing was lost while the clock stood still");
    // A late frame (a save loading) is not an absence.
    std::env::set_var("NGU_FIXED_NOW", (start + 4).to_string());
    session.advance(1.0 / 60.0);
    session.advance(1.0 / 60.0);
    assert_eq!(session.vm().eng.resumed, 0, "four seconds are not an absence");
    // The tab was hidden for an hour: the first frame back counts a quarter of a second, the clock says an hour.
    std::env::set_var("NGU_FIXED_NOW", (start + 4 + 3600).to_string());
    session.advance(3600.0);
    session.advance(1.0 / 60.0);
    assert_eq!(session.vm().eng.resumed, 1, "an hour away is applied once");
    session.advance(1.0 / 60.0);
    session.advance(1.0 / 60.0);
    assert_eq!(session.vm().eng.resumed, 1, "and not again");
    std::env::remove_var("NGU_FIXED_NOW");
}
