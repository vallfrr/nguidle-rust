//! High-level actions (keyboard, mouse, gamepad) applied to the game: focus navigation and confirmation.
use clr::Vm;

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
    /// previous / next menu (L / R)
    Prev,
    Next,
}

/// Applies an action; returns the click point (engine space, y up) for `Activate`.
pub fn apply(vm: &mut Vm, a: Action) -> Option<(f32, f32)> {
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
            vm.eng.focus = clr::value::Value::Null;
            vm.eng.focus_rect = None;
        }
        Action::Prev => return clr::focus::menu_step(vm, -1),
        Action::Next => return clr::focus::menu_step(vm, 1),
    }
    None
}
