//! Time the host was away: a hidden browser tab, a sleeping computer, a suspended console.
//!
//! The game itself only applies offline progress when a save is opened (`OpenFileDialog.loadintoGame`: elapsed seconds
//! since `lastTime`, at most a year, only above 10 seconds, then `Character.addOfflineProgress`). A host that is frozen
//! for a while would lose that time, because a step never counts more than a fraction of a second. Every simulation step
//! therefore looks at the wall clock; a gap above the game's own threshold is handed to the same
//! `addOfflineProgress`, as if the game had been closed and opened again, but without reloading anything.

use crate::value::Value;
use crate::Vm;

/// The game's own threshold (`loadintoGame` ignores 10 seconds and less).
pub const AWAY_AFTER: f64 = 10.0;
/// The game's own ceiling (`loadintoGame` limits the time to one year).
pub const AWAY_MAX: i64 = 31_536_000;

/// Seconds since 1970 as the game reads them (`DateTime.UtcNow`, so `NGU_FIXED_NOW` applies).
pub fn wall_seconds() -> f64 {
    match std::env::var("NGU_FIXED_NOW").ok().and_then(|v| v.parse::<u64>().ok()) {
        Some(seconds) => seconds as f64,
        None => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    }
}

/// Real time the host let slip since its previous frame: the wall clock moved further than the time the engine was
/// allowed to count. Small differences are clock and scheduling noise, not time away.
pub fn slipped(last: &mut Option<f64>, now: f64, counted: f64) -> f64 {
    let Some(before) = last.replace(now) else { return 0.0 };
    let excess = now - before - counted;
    if excess > SLIP_MIN {
        excess
    } else {
        0.0
    }
}

/// A frame's excess up to this is not time away: a heavy step (loading a save takes seconds in a browser), a late frame.
/// It is the game's own threshold, so what counts is always something the game would call an absence.
pub const SLIP_MIN: f64 = AWAY_AFTER;

impl Vm {
    /// A host calls this once per display frame, before the steps, with the seconds it lets the engine count. The wall
    /// clock decides how much real time went by: a hidden browser tab (the engine counts a quarter of a second at most
    /// when the page wakes up), a computer that slept or a console that was suspended (the host's own timer stops).
    pub fn host_frame(&mut self, counted: f32) {
        if self.eng.resume_off {
            return;
        }
        let lost = slipped(&mut self.eng.wall_seen, wall_seconds(), f64::from(counted));
        self.lose_time(lost);
    }

    /// Real time the game could not play. Once it adds up to more than the game ignores after opening a save, the next
    /// step applies it as offline progress.
    pub fn lose_time(&mut self, seconds: f64) {
        if seconds > 0.0 {
            self.eng.away_pending += seconds;
        }
    }

    /// The whole seconds to apply now, if the lost time is over the game's threshold.
    pub(crate) fn away_seconds(&mut self) -> Option<i64> {
        if self.eng.away_pending <= AWAY_AFTER {
            return None;
        }
        let seconds = (self.eng.away_pending as i64).min(AWAY_MAX);
        self.eng.away_pending = 0.0;
        Some(seconds)
    }

    /// Forgets the last wall-clock reading, so the next step starts a new measurement. Opening a save does it:
    /// `loadintoGame` has already applied the time since the save was written.
    pub fn forget_wall_clock(&mut self) {
        self.eng.wall_seen = None;
        self.eng.away_pending = 0.0;
    }

    /// Applies `seconds` of offline progress to the running game, the way the game does after opening a save:
    /// the game's own `addOfflineProgress` (the "You were offline" screen it opens is closed again), between the two
    /// refreshes of the item stats the game does around it. Returns false when the game is not running.
    pub fn apply_away(&mut self, seconds: i64) -> bool {
        use crate::dev::call;
        let Some(Value::Obj(_)) = crate::save_slots::game_component(self, "Character") else {
            return false;
        };
        let started = std::time::Instant::now();
        let seconds = seconds.min(AWAY_MAX).to_string();
        for step in [
            ("Character.inventoryController.updateItemStats", None),
            ("Character.inventoryController.updateBonuses", None),
            ("Character.addOfflineProgress", Some(seconds.as_str())),
            // The game opens its "You were offline" screen at the end of the calculation. After opening a save it tells the
            // player what happened; after a sleep or a hidden tab it only gets in the way, so it is closed again.
            ("Character.splashScreen.closeScreen", None),
            ("Character.inventoryController.updateItemStats", None),
            ("Character.inventoryController.updateBonuses", None),
            ("Character.buttons.updateButtons", None),
            ("Character.refreshMenus", None),
        ] {
            let args: Vec<&str> = step.1.into_iter().collect();
            let result = call(self, step.0, &args);
            if result.contains("EXCEPTION") || result.starts_with("ERROR") {
                eprintln!("[resume] {result}");
            }
        }
        self.eng.resumed += 1;
        crate::elog!("[resume] {seconds} s applied in {:?}", started.elapsed());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_time_the_engine_did_not_count_is_lost() {
        let mut last = None;
        assert_eq!(slipped(&mut last, 1000.0, 0.0), 0.0); // first reading
        assert_eq!(slipped(&mut last, 1000.017, 0.017), 0.0); // a normal frame
        assert_eq!(slipped(&mut last, 1000.2, 0.017), 0.0); // a late frame is noise
        let hidden = slipped(&mut last, 1000.2 + 3600.0, 0.25); // a tab hidden for an hour
        assert!((hidden - 3599.75).abs() < 1e-6);
        assert_eq!(slipped(&mut last, 1000.0, 0.017), 0.0); // the clock went back
        assert_eq!(slipped(&mut last, 1004.0, 0.25), 0.0); // a heavy step (a save loading) is not an absence
        assert_eq!(slipped(&mut last, 1014.2, 0.25), 0.0); // just under the game's threshold
    }
}
