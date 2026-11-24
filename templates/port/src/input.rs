//! The input side of the port. PORT: implement [`Source`] for your platform (window events, a gamepad, touch).

use clr::platform_api::{Action, Engine, MouseButton, Pointer};

/// One input event, already translated to the engine's vocabulary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The pointer moved or a button changed (device pixels, top-left origin).
    Pointer(Pointer),
    /// A navigation command.
    Action(Action),
    /// A scroll of the focused list, in game pixels.
    Scroll(f32, f32),
    /// A key changed (Unity `KeyCode`, see `clr::platform_api::keys`).
    Key {
        /// Unity key code.
        key: i32,
        /// Down or up.
        down: bool,
        /// Auto-repeat of a held key.
        repeat: bool,
    },
    /// A non-primary mouse button changed at `(x, y)` device pixels.
    Button {
        /// Device-pixel x.
        x: f32,
        /// Device-pixel y.
        y: f32,
        /// Which button.
        button: MouseButton,
        /// Down or up.
        down: bool,
    },
    /// The mouse wheel turned over `(x, y)` device pixels, by notches (positive = up).
    Wheel(f32, f32, f32),
}

/// Where a port's input comes from.
pub trait Source {
    /// Events since the last frame, in order.
    fn poll(&mut self) -> Vec<Event>;

    /// Edit a text field: show a keyboard or text box starting from `text`. Return the new content, or `None` to cancel.
    fn edit_text(&mut self, text: &str, max_len: Option<usize>, numeric: bool) -> Option<String>;
}

/// Give one event to the engine.
pub fn deliver<E: Engine>(engine: &mut E, event: Event) {
    match event {
        Event::Pointer(pointer) => engine.set_pointer(pointer),
        Event::Action(action) => engine.action(action),
        Event::Scroll(dx, dy) => engine.scroll(dx, dy),
        Event::Key { key, down, repeat } => engine.key(key, down, repeat),
        Event::Button { x, y, button, down } => engine.press(x, y, button, down),
        Event::Wheel(x, y, notches) => engine.wheel(x, y, notches),
    }
}

/// A stand-in source that plays a fixed list of events, one batch per frame, then does nothing.
/// PORT: replace it with the real thing.
pub struct Scripted {
    frames: std::collections::VecDeque<Vec<Event>>,
}

impl Scripted {
    /// A source that yields `frames[0]` on the first poll, `frames[1]` on the second, and so on.
    pub fn new(frames: Vec<Vec<Event>>) -> Self {
        Self {
            frames: frames.into(),
        }
    }
}

impl Source for Scripted {
    fn poll(&mut self) -> Vec<Event> {
        self.frames.pop_front().unwrap_or_default()
    }

    fn edit_text(&mut self, text: &str, _max_len: Option<usize>, _numeric: bool) -> Option<String> {
        Some(text.to_string())
    }
}
