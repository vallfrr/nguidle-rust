//! Gamepad input on Windows. D-pad and left stick move focus, the right stick
//! scrolls, south confirms, east cancels and the bumpers change menus.
//! Console mode (`docs/CONSOLE-MODE.md`): tapping L / R changes menu, holding L shows the menu list (released: open the
//! chosen menu), west is the game's right click.
use crate::actions::Action;
use gilrs::{Axis, Button, EventType, Gilrs};
use std::time::{Duration, Instant};

/// How long L must stay down before the menu list appears (a shorter press is a tap: previous menu).
const HOLD: Duration = Duration::from_millis(250);

pub struct Pad {
    g: Gilrs,
    held: Option<Action>,
    next_repeat: Instant,
    dropdown_scroll: Option<Action>,
    next_dropdown_scroll: Instant,
    last_poll: Instant,
    l_down: Option<Instant>,
    l_holding: bool,
}

impl Pad {
    pub fn new() -> Option<Pad> {
        Gilrs::new().ok().map(|g| Pad {
            g, held: None, next_repeat: Instant::now(), dropdown_scroll: None,
            next_dropdown_scroll: Instant::now(), last_poll: Instant::now(), l_down: None, l_holding: false,
        })
    }

    pub fn poll(&mut self, dropdown_open: bool, console: bool) -> Vec<Action> {
        let mut out = vec![];
        let now = Instant::now();
        while let Some(ev) = self.g.next_event() {
            match ev.event {
                EventType::ButtonPressed(b, _) => match b {
                    Button::South => out.push(Action::Activate),
                    Button::East => out.push(Action::Back),
                    Button::West if console => out.push(Action::Secondary),
                    Button::LeftTrigger if console => {
                        self.l_down = Some(now);
                        self.l_holding = false;
                    }
                    Button::LeftTrigger => out.push(Action::Prev),
                    Button::RightTrigger if !self.l_holding => out.push(Action::Next),
                    _ => {}
                },
                EventType::ButtonReleased(Button::LeftTrigger, _) if console => {
                    if self.l_holding {
                        out.push(Action::HoldEnd);
                    } else if self.l_down.is_some() {
                        out.push(Action::Prev);
                    }
                    self.l_down = None;
                    self.l_holding = false;
                }
                _ => {}
            }
        }
        // held direction (d-pad or left stick) with repeat
        let mut dir = None;
        let mut scroll = (0.0, 0.0);
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
                v.signum() * ((v.abs() - 0.22) / 0.78).clamp(0.0, 1.0)
            };
            let position = (stick(Axis::RightStickX, 1.0), stick(Axis::RightStickY, -1.0));
            if position != (0.0, 0.0) {
                scroll = position;
            }
        }
        // L held long enough, or a direction pressed while it is down: the menu list
        if let Some(t) = self.l_down {
            if !self.l_holding && (now - t >= HOLD || dir.is_some()) {
                self.l_holding = true;
                out.push(Action::HoldStart);
            }
        }
        let dt = (now - self.last_poll).as_secs_f32().min(0.05);
        self.last_poll = now;
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
        let direction = if !dropdown_open || scroll.1.abs() < 0.38 {
            None
        } else if scroll.1 > 0.0 {
            Some(Action::Down)
        } else {
            Some(Action::Up)
        };
        if let Some(direction) = direction {
            if self.dropdown_scroll != Some(direction) || now >= self.next_dropdown_scroll {
                self.next_dropdown_scroll = now + Duration::from_millis(82);
                out.push(direction);
            }
        }
        self.dropdown_scroll = direction;
        if !dropdown_open && scroll != (0.0, 0.0) {
            let distance = 390.0 * dt;
            out.push(Action::Scroll { x: -scroll.0 * distance, y: scroll.1 * distance });
        }
        out
    }
}
