//! Gamepad input on Windows: the raw buttons and sticks go to the engine's shared decoder (`clr::gamepad`), which decides taps,
//! holds, repeats and dead zones the same way on every host (`docs/M5-SPEC.md`).
use crate::actions::Action;
use clr::console_mode::Pad as Decoded;
use clr::gamepad::{button, Decoder, State};
use gilrs::{Axis, Button, Gilrs};
use std::time::Instant;

pub struct Pad {
    g: Gilrs,
    decoder: Decoder,
    last_poll: Instant,
}

fn action_of(pad: Decoded) -> Option<Action> {
    Some(match pad {
        Decoded::Dir('u') => Action::Up,
        Decoded::Dir('d') => Action::Down,
        Decoded::Dir('l') => Action::Left,
        Decoded::Dir('r') => Action::Right,
        Decoded::Dir(_) => return None,
        Decoded::Activate => Action::Activate,
        Decoded::Back => Action::Back,
        Decoded::Secondary => Action::Secondary,
        Decoded::Prev => Action::Prev,
        Decoded::Next => Action::Next,
        Decoded::HoldStart => Action::HoldStart,
        Decoded::HoldEnd => Action::HoldEnd,
        Decoded::Scroll(x, y) => Action::Scroll { x, y },
        Decoded::Info => Action::Info,
        Decoded::Settings => Action::Settings,
        Decoded::InfoStuff => Action::InfoStuff,
        Decoded::Shortcut(c) => Action::Shortcut(c),
    })
}

impl Pad {
    pub fn new() -> Option<Pad> {
        Gilrs::new().ok().map(|g| Pad { g, decoder: Decoder::default(), last_poll: Instant::now() })
    }

    pub fn poll(&mut self, dropdown_open: bool, console: bool) -> Vec<Action> {
        let now = Instant::now();
        let dt = (now - self.last_poll).as_secs_f32();
        self.last_poll = now;
        while self.g.next_event().is_some() {}
        let mut state = State::default();
        for (_, gp) in self.g.gamepads() {
            for (pad_button, bit) in [
                (Button::South, button::CROSS), (Button::East, button::CIRCLE), (Button::West, button::SQUARE),
                (Button::North, button::TRIANGLE), (Button::LeftTrigger, button::L), (Button::RightTrigger, button::R),
                (Button::Start, button::START), (Button::Select, button::SELECT), (Button::DPadUp, button::UP),
                (Button::DPadDown, button::DOWN), (Button::DPadLeft, button::LEFT), (Button::DPadRight, button::RIGHT),
            ] {
                if gp.is_pressed(pad_button) {
                    state.buttons |= bit;
                }
            }
            let left = (gp.value(Axis::LeftStickX), -gp.value(Axis::LeftStickY));
            let right = (gp.value(Axis::RightStickX), -gp.value(Axis::RightStickY));
            if left != (0.0, 0.0) {
                state.left = left;
            }
            if right != (0.0, 0.0) {
                state.right = right;
            }
        }
        self.decoder.update(state, dt, dropdown_open, console).into_iter().filter_map(action_of).collect()
    }
}
