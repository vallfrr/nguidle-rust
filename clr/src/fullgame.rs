//! The full-game script: plays the whole game through the engine and reports OK / KO / SKIP per step.
//!
//! `clr <dll> fullgame <prepared folder> [report file] [game hours]`, or [`run`] from a host or a test.
//! A step that needs a game class the loaded assembly does not have (a synthetic test game) is skipped, not failed.
//! The process exit code is non-zero when one step is KO. Every step also checks that no game exception was reported.

use crate::value::Value;
use crate::Vm;
use std::path::PathBuf;

/// Result of one step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Ko,
    Skip,
}

/// One line of the report.
#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub detail: String,
}

/// What to play.
pub struct Options {
    /// Hours of game time for the long progression (30 ticks per second of game time).
    pub hours: f32,
    /// Folder for the steps' temporary saves. Never the player's saves.
    pub workdir: PathBuf,
}

const TICK: f32 = 1.0 / 30.0;
const UNLOCK: &str = include_str!("../../dev/unlock.steps");

fn frames(vm: &mut Vm, n: usize) {
    for _ in 0..n {
        vm.frame(TICK);
    }
}

fn steps(vm: &mut Vm, list: &[&str]) {
    let owned: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    crate::script::run_steps(vm, &owned);
}

fn number(vm: &mut Vm, path: &str) -> Option<f64> {
    crate::dev::get(vm, path).trim().parse().ok()
}

fn has_class(vm: &Vm, name: &str) -> bool {
    crate::save_slots::game_component(vm, name).is_some()
}

fn base64_save(vm: &mut Vm) -> Result<String, String> {
    let c = crate::save_slots::game_component(vm, "ImportExport").ok_or("no ImportExport component")?;
    let v = crate::save_slots::game_call(vm, &c, "getBase64Data", vec![])?;
    crate::bcl_core::to_string(vm, &v).map_err(|_| "the save is not a string".to_string())
}

fn open_save(vm: &mut Vm, text: &str) -> Result<(), String> {
    crate::save_slots::load_into_game(vm, text)
}

fn load_base64(vm: &mut Vm, text: &str) -> Result<(), String> {
    let c = crate::save_slots::game_component(vm, "ImportExport").ok_or("no ImportExport component")?;
    crate::save_slots::game_call(vm, &c, "loadBase64ToData", vec![Value::str(text)]).map(|_| ())
}

struct Report {
    checks: Vec<Check>,
}

impl Report {
    fn add(&mut self, name: &str, status: Status, detail: impl Into<String>) {
        let detail = detail.into();
        println!("[fullgame] {:<5} {name}: {detail}", match status { Status::Ok => "OK", Status::Ko => "KO", Status::Skip => "SKIP" });
        self.checks.push(Check { name: name.into(), status, detail });
    }
    fn check(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        self.add(name, if ok { Status::Ok } else { Status::Ko }, detail);
    }
    fn skip(&mut self, name: &str, why: &str) {
        self.add(name, Status::Skip, why);
    }
}

/// Plays the game loaded in `vm` (scene loaded, `awake_all` done, a few frames run). Returns every step.
pub fn run(vm: &mut Vm, opts: &Options) -> Vec<Check> {
    let mut r = Report { checks: vec![] };
    let _ = std::fs::create_dir_all(&opts.workdir);
    let game = has_class(vm, "Character") && has_class(vm, "MenuSwapper") && has_class(vm, "ImportExport");
    let errors_before = vm.eng.exceptions;

    // 1. The title screen is up.
    let screen = crate::dev::ui(vm);
    r.check("title screen", !screen.trim().is_empty(), format!("{} lines on screen", screen.lines().count()));
    if !game {
        r.skip("game steps", "the loaded assembly is not the game (synthetic data)");
        r.check("no exception", vm.eng.exceptions == errors_before, format!("{} reported", vm.eng.exceptions - errors_before));
        return r.checks;
    }

    // 2. A new game starts and energy grows like the original (recorded: +58 in 60 s from 250).
    // The save picker's title screen (as in the applications): create a game in slot 1, then play.
    steps(vm, &["click,Create New Game", "click,Slot 1", "click,Create New Game", "w,30", "click,PLAY", "w,10"]);
    let start = number(vm, "Character.curEnergy");
    r.check("new game", start.is_some_and(|e| e >= 250.0), format!("energy {start:?}"));
    frames(vm, 1800);
    let after = number(vm, "Character.curEnergy");
    let gain = after.zip(start).map(|(a, s)| a - s);
    r.check("60 s of progress", gain.is_some_and(|g| (50.0..=66.0).contains(&g)), format!("gained {gain:?} (original: 58)"));

    // 3. Open every menu in the new game, as it is (the original raises nothing), then unlock every feature and do it again.
    let roots = crate::dev::menu_roots(vm);
    let before = vm.eng.exceptions;
    let (shown, empty) = open_every_menu(vm);
    r.check("menus of a new game without exception", roots.len() >= 50 && vm.eng.exceptions == before, format!("{shown} of {} menus opened, {} exceptions", roots.len(), vm.eng.exceptions - before));
    let lines: Vec<&str> = UNLOCK.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    steps(vm, &lines);
    steps(vm, &["ultimate"]);
    let before = vm.eng.exceptions;
    let (shown, empty) = open_every_menu(vm);
    r.check("menus", roots.len() >= 50 && empty.is_empty(), format!("{shown} of {} menus shown, empty: {empty:?}", roots.len()));
    r.check("menus without exception", vm.eng.exceptions == before, format!("{} exceptions", vm.eng.exceptions - before));

    // 4. A long progression, one hour at a time: no exception, the game stays valid and every hourly save loads back.
    let hours = opts.hours.max(0.0);
    let mut hour = 0.0f32;
    let mut last_save = String::new();
    let long_before = vm.eng.exceptions;
    let mut energy = number(vm, "Character.curEnergy").unwrap_or(0.0);
    let mut energy_ok = true;
    while hour < hours {
        let step = (hours - hour).min(1.0);
        frames(vm, (step * 3600.0 * 30.0) as usize);
        hour += step;
        match base64_save(vm) {
            Ok(s) if !s.is_empty() => last_save = s,
            Ok(_) | Err(_) => {
                energy_ok = false;
            }
        }
        let now = number(vm, "Character.curEnergy").unwrap_or(f64::NAN);
        if now.is_nan() || now < 0.0 {
            energy_ok = false;
        }
        energy = now;
    }
    r.check("long progression", energy_ok && vm.eng.exceptions == long_before, format!("{hours} h played, energy {energy}, {} exceptions", vm.eng.exceptions - long_before));
    let _ = std::fs::write(opts.workdir.join("long-run.sav"), &last_save);

    // 5. Save and load. The game itself adjusts a few fields on its first load, so the check is that a loaded game
    // saves back to the same bytes, however many times it is loaded.
    clock(1_700_000_000);
    match base64_save(vm) {
        Ok(first) => {
            let loaded = load_base64(vm, &first);
            let second = base64_save(vm);
            let reloaded = second.as_ref().map(|s| load_base64(vm, s));
            let third = base64_save(vm);
            let stable = second.is_ok() && second == third;
            r.check(
                "save round trip",
                loaded.is_ok() && matches!(reloaded, Ok(Ok(()))) && stable,
                format!("{} bytes, stable after load {stable}, first save and first reload {}", first.len(), if second.as_ref() == Ok(&first) { "identical" } else { "differ (the game's own load adjustments)" }),
            );
        }
        Err(e) => r.check("save round trip", false, e),
    }

    // The original computes (int)(float * float) in double precision: with both quest speed-ups the quest timer threshold
    // is 20303 (the original's value, read from its own saves), not 20304.
    steps(vm, &["set,Character.arbitrary.hasFasterQuests,true", "set,Character.inventory.itemList.fadComplete,true"]);
    let threshold = crate::dev::call(vm, "BeastQuestController.timerThreshold", &[]);
    r.check("float arithmetic like the original", threshold.ends_with("-> 20303"), threshold);

    // 6. Offline progress: a save loaded after an absence applies the game's own offline calculation.
    offline(vm, &mut r);

    // 7. Console mode: every focusable element answers the controller and nothing raises.
    console(vm, &mut r, roots.len());

    r.check("no exception", vm.eng.exceptions == errors_before, format!("{} reported in the whole run", vm.eng.exceptions - errors_before));
    r.checks
}

/// Opens every menu that is not hidden; returns how many, and the numbers of those that show nothing.
fn open_every_menu(vm: &mut Vm) -> (usize, Vec<usize>) {
    let roots = crate::dev::menu_roots(vm);
    let (mut shown, mut empty) = (0usize, vec![]);
    for n in 0..roots.len() {
        if crate::unity::with_go(&roots[n], |g| g.port_hidden).unwrap_or(false) {
            continue;
        }
        steps(vm, &[&format!("menu,{n}")]);
        shown += 1;
        if crate::dev::ui(vm).trim().is_empty() {
            empty.push(n);
        }
    }
    (shown, empty)
}

/// Sets the clock the game reads (`DateTime.UtcNow`), in seconds since 1970.
fn clock(seconds: u64) {
    std::env::set_var("NGU_FIXED_NOW", seconds.to_string());
}

/// Equal within 0.1 %: the game's offline formulas and its ticks differ by a few ticks, and a load shifts the tick phase.
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-3 * a.abs().max(b.abs())
}

/// Offline progress. For a simple state (all energy in attack training, no fight, no randomness) one hour of offline
/// progress must equal one hour played tick by tick (the original game gives identical numbers). A save loaded after an
/// absence applies it, whichever way the clock moved (the original uses the absolute difference).
fn offline(vm: &mut Vm, r: &mut Report) {
    let t0 = 1_700_000_000u64;
    let previous = std::env::var("NGU_FIXED_NOW").ok();
    // The clock is moved by hand below: the host-away catch-up is checked at the end, not on every jump.
    vm.eng.resume_off = true;
    clock(t0);
    steps(vm, &["set,Character.idleEnergy,100000", "set,Character.curEnergy,100000", "click,Basic Training", "w,5", "913,141", "w,5", "913,385", "w,60", &format!("set,Character.lastTime,{t0}")]);
    let Ok(start) = base64_save(vm) else {
        r.check("offline progress", false, "no save to start from");
        return;
    };
    let attack0 = number(vm, "Character.attack").unwrap_or(f64::NAN);
    // One hour played tick by tick (plus 3 s, like the offline branch below, so the game applies what it computed).
    // Both branches start from the same loaded save, so what a save does not keep cannot make them differ.
    let _ = load_base64(vm, &start);
    frames(vm, 1);
    frames(vm, 3600 * 30 + 90);
    let played = number(vm, "Character.attack").unwrap_or(f64::NAN);
    // The same hour computed by the game's own offline calculation.
    let _ = load_base64(vm, &start);
    frames(vm, 1);
    let direct = crate::dev::call(vm, "Character.addOfflineProgress", &["3600"]);
    frames(vm, 90);
    let computed = number(vm, "Character.attack").unwrap_or(f64::NAN);
    r.check("offline = one hour played", played > attack0 && near(played, computed), format!("attack {attack0} -> played {played}, offline {computed} ({direct})"));
    // A save loaded one hour later, and one hour earlier, gets the same progress.
    let mut loaded = vec![];
    for (label, at) in [("1 hour later", t0 + 3600), ("1 hour earlier", t0 - 3600)] {
        clock(t0);
        let _ = load_base64(vm, &start);
        frames(vm, 1);
        clock(at);
        let ok = open_save(vm, &start).is_ok();
        frames(vm, 90);
        let attack = number(vm, "Character.attack").unwrap_or(f64::NAN);
        loaded.push((label, ok, attack));
    }
    let both = loaded.iter().all(|(_, ok, a)| *ok && *a > attack0);
    let same = near(loaded[0].2, loaded[1].2) && near(loaded[0].2, computed);
    r.check("offline on load", both && same, loaded.iter().map(|(l, _, a)| format!("{l}: {a}")).collect::<Vec<_>>().join(", "));
    // Time the host was away (a hidden tab, a sleeping computer): the host reports an hour it could not play, and the next
    // step applies the game's own offline progress, so the result is that of the same hour computed above.
    vm.eng.resume_off = false;
    clock(t0);
    let _ = load_base64(vm, &start);
    vm.forget_wall_clock();
    frames(vm, 1);
    let applied_before = vm.eng.resumed;
    vm.lose_time(3600.0);
    frames(vm, 90);
    let away = number(vm, "Character.attack").unwrap_or(f64::NAN);
    let applied = vm.eng.resumed - applied_before;
    r.check("away time = offline progress", applied == 1 && near(away, computed), format!("applied {applied} time(s), attack {away} against {computed}"));
    // Ten seconds or less is not away time (the game ignores that much after opening a save too).
    let applied_before = vm.eng.resumed;
    vm.lose_time(10.0);
    frames(vm, 2);
    r.check("ten seconds are not away time", vm.eng.resumed == applied_before, format!("{} applied", vm.eng.resumed - applied_before));
    // The longest absence the game accepts (a year) must be quick: this is what a player waits for when the host wakes up.
    vm.lose_time(63_072_000.0);
    let started = std::time::Instant::now();
    frames(vm, 1);
    let millis = started.elapsed().as_millis();
    r.check("a year away is applied in under a second", vm.eng.resumed > applied_before && millis < 1000, format!("{millis} ms"));
    // The game's "You were offline" screen is for opening a save; time away leaves nothing on the screen.
    r.check("no offline screen after time away", !crate::dev::ui(vm).contains("You were offline"), "screen text checked");
    match previous {
        Some(v) => std::env::set_var("NGU_FIXED_NOW", v),
        None => std::env::remove_var("NGU_FIXED_NOW"),
    }
    vm.forget_wall_clock();
    vm.eng.resume_off = false;
}

/// Visits every element the controller can focus on every menu: activates it, goes back, and checks nothing raises.
fn console(vm: &mut Vm, r: &mut Report, menus: usize) {
    use crate::console_mode::Pad;
    let before = vm.eng.exceptions;
    steps(vm, &["consolemode,on"]);
    if vm.eng.console.is_none() {
        r.check("console mode", false, "console mode did not start");
        return;
    }
    let (mut elements, mut menus_walked) = (0usize, 0usize);
    for _ in 0..menus.max(1) {
        let mut seen = std::collections::BTreeSet::new();
        let mut idle = 0;
        // Move down until no new element comes up for 6 moves in a row.
        for _ in 0..120 {
            let Some(mut c) = vm.eng.console.take() else { break };
            let _ = c.input(vm, Pad::Dir('d'));
            vm.eng.console = Some(c);
            crate::console_mode::sync(vm);
            match crate::focus::focused_label(vm) {
                Some(label) => {
                    if seen.insert(label) {
                        idle = 0;
                        steps(vm, &["pad,a", "pad,b"]);
                    } else {
                        idle += 1;
                    }
                }
                None => idle += 1,
            }
            frames(vm, 2);
            if idle >= 6 {
                break;
            }
        }
        elements += seen.len();
        menus_walked += 1;
        steps(vm, &["pad,next"]);
    }
    r.check(
        "console mode",
        elements >= 20 && vm.eng.exceptions == before,
        format!("{elements} elements focused and activated over {menus_walked} menus, {} exceptions", vm.eng.exceptions - before),
    );
}

/// The report as text, and whether every step passed.
pub fn report(checks: &[Check]) -> (String, bool) {
    let mut out = String::new();
    let (mut ok, mut ko, mut skip) = (0, 0, 0);
    for c in checks {
        let tag = match c.status {
            Status::Ok => {
                ok += 1;
                "OK  "
            }
            Status::Ko => {
                ko += 1;
                "KO  "
            }
            Status::Skip => {
                skip += 1;
                "SKIP"
            }
        };
        out += &format!("{tag} {}: {}\n", c.name, c.detail);
    }
    out += &format!("FULL GAME: {ok} OK, {ko} KO, {skip} SKIP\n");
    (out, ko == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, status: Status) -> Check {
        Check { name: name.into(), status, detail: "detail".into() }
    }

    #[test]
    fn the_report_counts_every_step_and_fails_on_a_ko() {
        let (text, ok) = report(&[check("a", Status::Ok), check("b", Status::Skip)]);
        assert!(ok && text.ends_with("FULL GAME: 1 OK, 0 KO, 1 SKIP\n"), "{text}");
        let (text, ok) = report(&[check("a", Status::Ok), check("b", Status::Ko)]);
        assert!(!ok && text.contains("KO   b: detail") && text.ends_with("FULL GAME: 1 OK, 1 KO, 0 SKIP\n"), "{text}");
    }

    #[test]
    fn offline_numbers_are_compared_with_a_small_tolerance() {
        assert!(near(125_971_416.03, 125_966_875.03));
        assert!(!near(1_020_781_321.0, 135_894_213_798_093.5));
    }
}
