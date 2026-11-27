//! Gamepad decoding shared by every host (`docs/M5-SPEC.md`): the host reads its buttons and sticks once per display frame
//! and gets back the decoded events (taps and holds of L and R, repeats of the directions, stick dead zones), so that the
//! console mode behaves the same on the Vita, the PC window and the browser.
//!
//! Outside the console mode the decoding is the original one: cross confirms, circle goes back, L / R change menu,
//! the d-pad and left stick move the selection, the right stick scrolls.
use crate::console_mode::Pad;

/// Button bits of [`State::buttons`].
pub mod button {
    pub const CROSS: u32 = 1 << 0;
    pub const CIRCLE: u32 = 1 << 1;
    pub const SQUARE: u32 = 1 << 2;
    pub const TRIANGLE: u32 = 1 << 3;
    pub const L: u32 = 1 << 4;
    pub const R: u32 = 1 << 5;
    pub const START: u32 = 1 << 6;
    pub const SELECT: u32 = 1 << 7;
    pub const UP: u32 = 1 << 8;
    pub const DOWN: u32 = 1 << 9;
    pub const LEFT: u32 = 1 << 10;
    pub const RIGHT: u32 = 1 << 11;
    pub const FACE: u32 = CROSS | CIRCLE | SQUARE | TRIANGLE;
    pub const DPAD: u32 = UP | DOWN | LEFT | RIGHT;
}
use button::*;

/// What the host read this frame. Sticks are in -1..1, x positive to the right, y positive towards the bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub buttons: u32,
    pub left: (f32, f32),
    pub right: (f32, f32),
}

/// L or R held this long is a hold, not a tap.
const HOLD_SECONDS: f64 = 0.25;
const REPEAT_FIRST: f64 = 0.4;
const REPEAT: f64 = 0.12;
const DROPDOWN_REPEAT: f64 = 0.082;
/// Right stick: dead zone and speed (game pixels per second at full deflection).
const DEAD_ZONE: f32 = 0.22;
const SCROLL_SPEED: f32 = 390.0;

/// The tap / hold gesture of L or R: a tap acts on release, a hold acts while it lasts.
#[derive(Default)]
struct Shoulder {
    since: Option<f64>,
    /// held long enough, or another button was used with it
    holding: bool,
}

impl Shoulder {
    fn down(&self) -> bool {
        self.since.is_some()
    }
}

#[derive(Default)]
pub struct Decoder {
    now: f64,
    previous: u32,
    direction: Option<char>,
    next_repeat: f64,
    l: Shoulder,
    r: Shoulder,
    dropdown_direction: Option<char>,
    next_dropdown: f64,
}

fn direction_of(state: &State) -> Option<char> {
    let (x, y) = state.left;
    if state.buttons & UP != 0 || y < -0.5 {
        Some('u')
    } else if state.buttons & DOWN != 0 || y > 0.5 {
        Some('d')
    } else if state.buttons & LEFT != 0 || x < -0.5 {
        Some('l')
    } else if state.buttons & RIGHT != 0 || x > 0.5 {
        Some('r')
    } else {
        None
    }
}

fn shortcut_of(edge: u32) -> Vec<char> {
    [(CROSS, 'x'), (CIRCLE, 'o'), (SQUARE, 's'), (TRIANGLE, 't'), (UP, 'u'), (DOWN, 'd'), (LEFT, 'l'), (RIGHT, 'r')]
        .into_iter()
        .filter(|(bit, _)| edge & bit != 0)
        .map(|(_, c)| c)
        .collect()
}

impl Decoder {
    /// Decodes one frame. `dt` is the time since the previous call.
    pub fn update(&mut self, state: State, dt: f32, dropdown_open: bool, console: bool) -> Vec<Pad> {
        self.now += f64::from(dt.clamp(0.0, 0.1));
        let now = self.now;
        let edge = state.buttons & !self.previous;
        let released = self.previous & !state.buttons;
        self.previous = state.buttons;
        let mut out = vec![];
        let direction = direction_of(&state);

        if !console {
            self.l = Shoulder::default();
            self.r = Shoulder::default();
            if edge & CROSS != 0 {
                out.push(Pad::Activate);
            }
            if edge & CIRCLE != 0 {
                out.push(Pad::Back);
            }
            if edge & L != 0 {
                out.push(Pad::Prev);
            }
            if edge & R != 0 {
                out.push(Pad::Next);
            }
        } else {
            // L: tap = previous menu; hold (or a direction while it is down) = the menu list
            if edge & L != 0 {
                self.l.since.get_or_insert(now);
            }
            if let Some(since) = self.l.since {
                if !self.l.holding && (direction.is_some() || now - since >= HOLD_SECONDS) {
                    self.l.holding = true;
                    out.push(Pad::HoldStart);
                }
            }
            if released & L != 0 && self.l.down() {
                out.push(if self.l.holding { Pad::HoldEnd } else { Pad::Prev });
                self.l = Shoulder::default();
            }
            // R: tap = next menu; held (or another button used with it) = the game's keyboard shortcuts
            if edge & R != 0 && !self.l.holding {
                self.r.since.get_or_insert(now);
            }
            if let Some(since) = self.r.since {
                let used = edge & (FACE | DPAD) != 0;
                if !self.r.holding && (used || now - since >= HOLD_SECONDS) {
                    self.r.holding = true;
                }
                if used {
                    out.extend(shortcut_of(edge & (FACE | DPAD)).into_iter().map(Pad::Shortcut));
                }
            }
            if released & R != 0 && self.r.down() {
                if !self.r.holding {
                    out.push(Pad::Next);
                }
                self.r = Shoulder::default();
            }
            let layer = self.r.holding || self.l.holding;
            if !layer {
                if edge & CROSS != 0 {
                    out.push(Pad::Activate);
                }
                if edge & CIRCLE != 0 {
                    out.push(Pad::Back);
                }
                if edge & SQUARE != 0 {
                    out.push(Pad::Secondary);
                }
                if edge & TRIANGLE != 0 {
                    out.push(Pad::Info);
                }
                if edge & START != 0 {
                    out.push(Pad::Settings);
                }
                if edge & SELECT != 0 {
                    out.push(Pad::InfoStuff);
                }
            } else if self.l.holding && edge & CIRCLE != 0 {
                out.push(Pad::Back);
            }
        }

        // the selection moves with the d-pad or left stick, repeating while held; not while R is a shortcut layer
        let moving = if console && self.r.holding { None } else { direction };
        match moving {
            Some(d) if self.direction != Some(d) => {
                self.direction = Some(d);
                self.next_repeat = now + REPEAT_FIRST;
                out.push(Pad::Dir(d));
            }
            Some(d) if now >= self.next_repeat => {
                self.next_repeat = now + REPEAT;
                out.push(Pad::Dir(d));
            }
            None => self.direction = None,
            _ => {}
        }

        // right stick: moves through an open dropdown, otherwise scrolls
        let stick = |v: f32| v.signum() * ((v.abs() - DEAD_ZONE) / (1.0 - DEAD_ZONE)).clamp(0.0, 1.0);
        let (rx, ry) = (stick(state.right.0), stick(state.right.1));
        let dropdown = if !dropdown_open || state.right.1.abs() < 0.38 {
            None
        } else if state.right.1 > 0.0 {
            Some('d')
        } else {
            Some('u')
        };
        if let Some(d) = dropdown {
            if self.dropdown_direction != Some(d) || now >= self.next_dropdown {
                self.next_dropdown = now + DROPDOWN_REPEAT;
                out.push(Pad::Dir(d));
            }
        }
        self.dropdown_direction = dropdown;
        if !dropdown_open && (rx != 0.0 || ry != 0.0) {
            let distance = SCROLL_SPEED * dt.clamp(0.0, 0.1);
            out.push(Pad::Scroll(-rx * distance, ry * distance));
        }
        out
    }

    /// Forgets what was held (the host lost focus, a screen opened).
    pub fn reset(&mut self) {
        *self = Decoder { now: self.now, ..Default::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(decoder: &mut Decoder, buttons: u32, dt: f32) -> Vec<Pad> {
        decoder.update(State { buttons, ..Default::default() }, dt, false, true)
    }

    #[test]
    fn l_and_r_tap_change_menu_and_hold_does_the_layer() {
        let mut d = Decoder::default();
        assert!(press(&mut d, L, 0.016).is_empty());
        assert_eq!(press(&mut d, 0, 0.016), vec![Pad::Prev]);
        assert!(press(&mut d, R, 0.016).is_empty());
        assert_eq!(press(&mut d, 0, 0.016), vec![Pad::Next]);
        // L held: the menu list until release
        press(&mut d, L, 0.016);
        let mut events = vec![];
        for _ in 0..20 {
            events.extend(press(&mut d, L, 0.016));
        }
        assert_eq!(events, vec![Pad::HoldStart]);
        assert_eq!(press(&mut d, 0, 0.016), vec![Pad::HoldEnd]);
    }

    #[test]
    fn r_with_another_button_gives_a_shortcut_and_never_changes_menu() {
        let mut d = Decoder::default();
        press(&mut d, R, 0.016);
        assert_eq!(press(&mut d, R | SQUARE, 0.016), vec![Pad::Shortcut('s')]);
        assert!(press(&mut d, R, 0.016).is_empty());
        assert_eq!(press(&mut d, 0, 0.016), vec![]);
        // the face button alone does its normal job
        assert_eq!(press(&mut d, CROSS, 0.016), vec![Pad::Activate]);
        // held long enough without another button: still a layer, released without a menu change
        let mut d = Decoder::default();
        for _ in 0..20 {
            press(&mut d, R, 0.016);
        }
        assert_eq!(press(&mut d, R | TRIANGLE, 0.016), vec![Pad::Shortcut('t')]);
        assert_eq!(press(&mut d, 0, 0.016), vec![]);
    }

    #[test]
    fn start_and_select_open_settings_and_info_and_the_direction_repeats() {
        let mut d = Decoder::default();
        assert_eq!(press(&mut d, START, 0.016), vec![Pad::Settings]);
        assert_eq!(press(&mut d, SELECT, 0.016), vec![Pad::InfoStuff]);
        assert_eq!(press(&mut d, TRIANGLE, 0.016), vec![Pad::Info]);
        let mut d = Decoder::default();
        assert_eq!(press(&mut d, DOWN, 0.016), vec![Pad::Dir('d')]);
        let mut repeats = 0;
        for _ in 0..40 {
            repeats += press(&mut d, DOWN, 0.016).len();
        }
        assert!(repeats >= 2 && repeats <= 4, "{repeats}");
    }

    #[test]
    fn outside_the_console_mode_it_is_the_original_decoding() {
        let mut d = Decoder::default();
        let state = |buttons| State { buttons, ..Default::default() };
        assert_eq!(d.update(state(L), 0.016, false, false), vec![Pad::Prev]);
        assert_eq!(d.update(state(R), 0.016, false, false), vec![Pad::Next]);
        assert_eq!(d.update(state(CROSS), 0.016, false, false), vec![Pad::Activate]);
        assert!(d.update(state(SQUARE | TRIANGLE | START), 0.016, false, false).is_empty());
    }
}
