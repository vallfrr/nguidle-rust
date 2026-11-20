//! Gamepad (gilrs). **NOT COMPILED / NOT TESTED**: `libudev` is absent from the Linux development machine;
//! to compile and check on Windows (XInput/WGI) or with `libudev-dev`. Mapping approved by the maintainer:
//! d-pad and left stick = move the focus (same behaviour, repeat after 0.4 s then every 0.12 s),
//! Cross (south) = confirm, Circle (east) = back, L1/R1 = previous / next menu, Start = settings, Select = info (to wire).
use crate::actions::Action;
use gilrs::{Axis, Button, EventType, Gilrs};
use std::time::{Duration, Instant};

pub struct Pad {
    g: Gilrs,
    held: Option<Action>,
    next_repeat: Instant,
}

impl Pad {
    pub fn new() -> Option<Pad> {
        Gilrs::new().ok().map(|g| Pad { g, held: None, next_repeat: Instant::now() })
    }

    pub fn poll(&mut self) -> Vec<Action> {
        let mut out = vec![];
        while let Some(ev) = self.g.next_event() {
            if let EventType::ButtonPressed(b, _) = ev.event {
                match b {
                    Button::South => out.push(Action::Activate),
                    Button::East => out.push(Action::Back),
                    Button::LeftTrigger => out.push(Action::Prev),
                    Button::RightTrigger => out.push(Action::Next),
                    _ => {}
                }
            }
        }
        // held direction (d-pad or left stick) with repeat
        let mut dir = None;
        for (_, gp) in self.g.gamepads() {
            let (x, y) = (gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
            if gp.is_pressed(Button::DPadUp) || y > 0.5 {
                dir = Some(Action::Up);
            } else if gp.is_pressed(Button::DPadDown) || y < -0.5 {
                dir = Some(Action::Down);
            } else if gp.is_pressed(Button::DPadLeft) || x < -0.5 {
                dir = Some(Action::Left);
            } else if gp.is_pressed(Button::DPadRight) || x > 0.5 {
                dir = Some(Action::Right);
            }
        }
        let now = Instant::now();
        match dir {
            Some(d) if self.held != Some(d) => {
                self.held = Some(d);
                self.next_repeat = now + Duration::from_millis(400);
                out.push(d);
            }
            Some(d) if now >= self.next_repeat => {
                self.next_repeat = now + Duration::from_millis(120);
                out.push(d);
            }
            None => self.held = None,
            _ => {}
        }
        out
    }
}
