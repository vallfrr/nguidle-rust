//! High-level actions (keyboard, mouse, gamepad) applied to the game: focus navigation and confirmation.
use clr::Vm;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    /// confirm (cross): virtual click at the centre of the focused element
    Activate,
    /// back (circle): removes the focus
    Back,
    /// Scroll the currently focused viewport.
    Scroll { x: f32, y: f32 },
    /// previous / next menu (L / R)
    Prev,
    Next,
    /// console mode: the game's right click on the focused control (square)
    Secondary,
    /// console mode: L held / released (menu list)
    HoldStart,
    HoldEnd,
}

/// The shared tap/hold gesture for the keyboard and controller menu shortcuts.
#[derive(Default)]
pub(crate) struct MenuShoulder {
    pressed: Option<Instant>,
    pub holding: bool,
}

impl MenuShoulder {
    pub fn press(&mut self, now: Instant) {
        self.pressed.get_or_insert(now);
    }

    pub fn is_down(&self) -> bool {
        self.pressed.is_some()
    }

    pub fn poll(&mut self, now: Instant, navigating: bool) -> Option<Action> {
        let since = self.pressed?;
        if !self.holding && (navigating || now.duration_since(since) >= Duration::from_millis(250)) {
            self.holding = true;
            return Some(Action::HoldStart);
        }
        None
    }

    pub fn release(&mut self) -> Option<Action> {
        let was_holding = std::mem::take(&mut self.holding);
        self.pressed.take()?;
        Some(if was_holding { Action::HoldEnd } else { Action::Prev })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_shoulder_taps_once_and_holds_until_release() {
        let now = Instant::now();
        let mut shoulder = MenuShoulder::default();
        shoulder.press(now);
        assert_eq!(shoulder.poll(now + Duration::from_millis(249), false), None);
        assert_eq!(shoulder.release(), Some(Action::Prev));
        assert_eq!(shoulder.release(), None);

        shoulder.press(now);
        shoulder.press(now + Duration::from_millis(200));
        assert_eq!(shoulder.poll(now + Duration::from_millis(250), false), Some(Action::HoldStart));
        assert_eq!(shoulder.poll(now + Duration::from_millis(500), true), None);
        assert_eq!(shoulder.release(), Some(Action::HoldEnd));

        shoulder.press(now);
        assert_eq!(shoulder.poll(now, true), Some(Action::HoldStart));
        assert_eq!(shoulder.release(), Some(Action::HoldEnd));
    }
}

/// The same input for the console presentation (`clr::console_mode`).
pub fn console_pad(a: Action) -> clr::console_mode::Pad {
    use clr::console_mode::Pad;
    match a {
        Action::Up => Pad::Dir('u'),
        Action::Down => Pad::Dir('d'),
        Action::Left => Pad::Dir('l'),
        Action::Right => Pad::Dir('r'),
        Action::Activate => Pad::Activate,
        Action::Back => Pad::Back,
        Action::Scroll { x, y } => Pad::Scroll(x, y),
        Action::Prev => Pad::Prev,
        Action::Next => Pad::Next,
        Action::Secondary => Pad::Secondary,
        Action::HoldStart => Pad::HoldStart,
        Action::HoldEnd => Pad::HoldEnd,
    }
}

/// Applies an action; returns the click point (engine space, y up) for `Activate`.
pub fn apply(vm: &mut Vm, a: Action) -> Option<(f32, f32)> {
    let modal = match a {
        Action::Up => 'u',
        Action::Down => 'd',
        Action::Activate => 'a',
        Action::Back => 'b',
        _ => ' ',
    };
    if vm.dropdown_navigate(modal) {
        return None;
    }
    match a {
        Action::Up => {
            clr::focus::navigate(vm, 'u');
        }
        Action::Down => {
            clr::focus::navigate(vm, 'd');
        }
        Action::Left => {
            clr::focus::navigate(vm, 'l');
        }
        Action::Right => {
            clr::focus::navigate(vm, 'r');
        }
        Action::Activate => return clr::focus::activation_point(vm),
        Action::Back => {
            if !vm.back_from_desktop_picker() {
                clr::focus::clear(vm);
            }
        }
        Action::Scroll { x, y } => clr::focus::scroll(vm, x, y),
        Action::Prev => return clr::focus::menu_step(vm, -1),
        Action::Next => return clr::focus::menu_step(vm, 1),
        Action::Secondary | Action::HoldStart | Action::HoldEnd => {}
    }
    None
}
