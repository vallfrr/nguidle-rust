//! Gamepad input on Windows. D-pad and left stick move focus, the right stick
//! scrolls, south confirms, east cancels and the bumpers change menus.
use crate::actions::Action;
use gilrs::{Axis, Button, EventType, Gilrs};
use std::time::{Duration, Instant};

pub struct Pad {
    g: Gilrs,
    held: Option<Action>,
    next_repeat: Instant,
    scroll: (i8, i8),
    next_scroll: Instant,
}

impl Pad {
    pub fn new() -> Option<Pad> {
        Gilrs::new().ok().map(|g| Pad {
            g, held: None, next_repeat: Instant::now(), scroll: (0, 0), next_scroll: Instant::now(),
        })
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
        let mut scroll = (0, 0);
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
            let stick = |axis, sign: f32| {
                let v = gp.value(axis) * sign;
                if v.abs() > 0.52 { v.signum() as i8 } else { 0 }
            };
            let position = (stick(Axis::RightStickX, 1.0), stick(Axis::RightStickY, -1.0));
            if position != (0, 0) {
                scroll = position;
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
        if scroll != (0, 0) && (scroll != self.scroll || now >= self.next_scroll) {
            self.next_scroll = now + Duration::from_millis(82);
            out.push(Action::Scroll { x: scroll.0, y: scroll.1 });
        }
        self.scroll = scroll;
        out
    }
}
