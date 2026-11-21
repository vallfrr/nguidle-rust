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
    /// Scroll the currently focused viewport.
    Scroll { x: i8, y: i8 },
    /// previous / next menu (L / R)
    Prev,
    Next,
}

/// Applies an action; returns the click point (engine space, y up) for `Activate`.
pub fn apply(vm: &mut Vm, a: Action) -> Option<(f32, f32)> {
    let modal = match a {
        Action::Up | Action::Scroll { y: -1, .. } => 'u',
        Action::Down | Action::Scroll { y: 1, .. } => 'd',
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
        Action::Scroll { x, y } => clr::focus::scroll(vm, -f32::from(x) * 32.0, f32::from(y) * 32.0),
        Action::Prev => return clr::focus::menu_step(vm, -1),
        Action::Next => return clr::focus::menu_step(vm, 1),
    }
    None
}
