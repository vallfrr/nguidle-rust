//! Platform interface of the NGU Idle engine.
//!
//! A **port** (desktop, web, console...) hosts the engine: it gives it time and input, and draws what
//! the engine produces. This crate is the contract between the two. It has no dependency on the engine
//! or on a graphics API, so a port can be written and tested against it alone.
//!
//! * [`Engine`] is what a port drives. The engine crate implements it (`clr::session::Session`);
//!   [`testing::ScriptedEngine`] is a synthetic implementation for tests that need no game files.
//! * [`DrawTarget`] is what a port provides to receive a frame, as ordered [`Quad`]s. [`TriangleList`]
//!   converts them to vertex and batch arrays, the layout the C ABI and the browser adapter expose.
//! * [`FixedStep`] is the logic-time accumulator every port needs: the game logic advances in fixed
//!   steps whatever the display rate.
//!
//! Threading: an [`Engine`] is single-threaded. Call every method from the one thread that created it,
//! never re-entrantly. Borrowed data (textures, draw buffers) is valid until the next call.

#![deny(missing_docs)]

pub mod testing;

/// Duration of one game logic step, in seconds. The game logic always advances in whole steps of this
/// length, so its state does not depend on the display rate.
pub const STEP_SECONDS: f32 = 1.0 / 30.0;

/// Reference width of the game's screen, in game pixels.
pub const GAME_WIDTH: f32 = 960.0;

/// Reference height of the game's screen, in game pixels.
pub const GAME_HEIGHT: f32 = 600.0;

/// Texture identifier of the engine's glyph atlas (see [`Engine::texture`]).
pub const TEXTURE_ATLAS: u32 = 1;

/// A textured rectangle in draw order, in device pixels with a top-left origin.
#[derive(Clone)]
pub struct Quad {
    /// Screen-space corners in origin, +u, +u+v, +v order.
    pub p: [[f32; 2]; 4],
    /// Texture coordinates with V increasing downwards, in the same order.
    pub uv: [[f32; 2]; 4],
    /// Colour multiplier, RGBA in `0.0..=1.0`.
    pub col: [f32; 4],
    /// Texture identifier: `0` is a plain white texel, [`TEXTURE_ATLAS`] the glyph atlas, others come
    /// from [`Engine::texture`].
    pub tex: u32,
    /// Sample with nearest-neighbour filtering instead of bilinear.
    pub nearest: bool,
    /// Scissor rectangle `[x0, y0, x1, y1]` in device pixels; the quad is clipped to it.
    pub clip: [i32; 4],
}

/// One vertex of a [`TriangleList`]. `repr(C)`, 20 bytes: this is the layout shared with C hosts.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Device-pixel x.
    pub x: f32,
    /// Device-pixel y, top-left origin.
    pub y: f32,
    /// Texture coordinate u.
    pub u: f32,
    /// Texture coordinate v, increasing downwards.
    pub v: f32,
    /// RGBA bytes ordered from the least significant to the most significant.
    pub rgba: u32,
}

/// A run of consecutive vertices that share render state. `repr(C)`, 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch {
    /// Texture identifier, as in [`Quad::tex`].
    pub tex: u32,
    /// Non-zero to sample with nearest-neighbour filtering.
    pub nearest: u32,
    /// Scissor rectangle `[x0, y0, x1, y1]` in device pixels.
    pub clip: [i32; 4],
    /// Index of the first vertex of the run.
    pub first: u32,
    /// Number of vertices of the run (a multiple of 6: two triangles per quad).
    pub count: u32,
}

/// Reuse the output buffers and batch only consecutive quads with identical
/// render settings. Preserving draw order is necessary for blended layers.
pub fn tessellate(quads: &[Quad], vertices: &mut Vec<Vertex>, batches: &mut Vec<Batch>) {
    vertices.clear();
    batches.clear();
    for quad in quads {
        let first = vertices.len() as u32;
        let rgba = u32::from_le_bytes(
            quad.col
                .map(|channel| (channel.clamp(0.0, 1.0) * 255.0 + 0.5) as u8),
        );
        for index in [0, 1, 2, 0, 2, 3] {
            let [x, y] = quad.p[index];
            let [u, v] = quad.uv[index];
            vertices.push(Vertex { x, y, u, v, rgba });
        }
        let nearest = u32::from(quad.nearest);
        match batches.last_mut() {
            Some(batch)
                if (batch.tex, batch.nearest, batch.clip) == (quad.tex, nearest, quad.clip) =>
            {
                batch.count += 6;
            }
            _ => batches.push(Batch {
                tex: quad.tex,
                nearest,
                clip: quad.clip,
                first,
                count: 6,
            }),
        }
    }
}

/// Receives an ordered frame in logical draw order.
pub trait DrawTarget {
    /// Draw `quads` in order, each over the previous ones. Called once per frame by [`Engine::draw`].
    fn draw_quads(&mut self, quads: &[Quad]);
}

/// Reusable triangle buffers for renderers that batch textured geometry.
#[derive(Default)]
pub struct TriangleList {
    /// Vertices of the last frame, six per quad.
    pub vertices: Vec<Vertex>,
    /// Draw runs of the last frame, in order.
    pub batches: Vec<Batch>,
}

/// Starting value of [`TriangleList::fingerprint`].
pub const FINGERPRINT_SEED: u64 = 0xcbf2_9ce4_8422_2325;

impl TriangleList {
    /// Fold every vertex of the current frame into `hash` (FNV-1a over the little-endian bytes of the fields) and return it.
    /// Start from [`FINGERPRINT_SEED`] and feed each frame in turn: two runs that draw the same frames give the same value,
    /// so a port can compare its draw list with the reference host's.
    pub fn fingerprint(&self, mut hash: u64) -> u64 {
        for vertex in &self.vertices {
            let words = [
                vertex.x.to_bits(),
                vertex.y.to_bits(),
                vertex.u.to_bits(),
                vertex.v.to_bits(),
                vertex.rgba,
            ];
            for byte in words.iter().flat_map(|word| word.to_le_bytes()) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }
}

impl DrawTarget for TriangleList {
    fn draw_quads(&mut self, quads: &[Quad]) {
        tessellate(quads, &mut self.vertices, &mut self.batches);
    }
}

/// Unity `KeyCode` values for [`Engine::key`]. Letters are the lower-case ASCII codes (`97` is `a`) and digits their ASCII codes
/// (`48` is `0`); only the keys the game reads are listed here.
pub mod keys {
    /// Backspace.
    pub const BACKSPACE: i32 = 8;
    /// Tab.
    pub const TAB: i32 = 9;
    /// Enter.
    pub const ENTER: i32 = 13;
    /// Escape.
    pub const ESCAPE: i32 = 27;
    /// Space.
    pub const SPACE: i32 = 32;
    /// Up arrow.
    pub const UP: i32 = 273;
    /// Down arrow.
    pub const DOWN: i32 = 274;
    /// Right arrow.
    pub const RIGHT: i32 = 275;
    /// Left arrow.
    pub const LEFT: i32 = 276;
    /// Right Shift.
    pub const RIGHT_SHIFT: i32 = 303;
    /// Left Shift.
    pub const LEFT_SHIFT: i32 = 304;
    /// Right Control.
    pub const RIGHT_CTRL: i32 = 305;
    /// Left Control.
    pub const LEFT_CTRL: i32 = 306;
    /// Right Alt.
    pub const RIGHT_ALT: i32 = 307;
    /// Left Alt.
    pub const LEFT_ALT: i32 = 308;
}

/// A navigation command from a keyboard, gamepad or remote. The engine moves a focus between controls
/// and can press the focused one, so a port with no pointer can play the whole game.
///
/// The C ABI passes these as the character returned by [`Action::code`] (`ngu_pad`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Move the focus up.
    Up,
    /// Move the focus down.
    Down,
    /// Move the focus left.
    Left,
    /// Move the focus right.
    Right,
    /// Press the focused control (a click at its centre).
    Activate,
    /// Close the open dropdown or picker, otherwise drop the focus.
    Back,
    /// Show the previous menu.
    Prev,
    /// Show the next menu.
    Next,
}

impl Action {
    /// The single-character code used by the C ABI: `u d l r a b p n`.
    pub fn code(self) -> char {
        match self {
            Action::Up => 'u',
            Action::Down => 'd',
            Action::Left => 'l',
            Action::Right => 'r',
            Action::Activate => 'a',
            Action::Back => 'b',
            Action::Prev => 'p',
            Action::Next => 'n',
        }
    }

    /// The action for a C ABI code, or `None` for any other character.
    pub fn from_code(code: char) -> Option<Action> {
        Some(match code {
            'u' => Action::Up,
            'd' => Action::Down,
            'l' => Action::Left,
            'r' => Action::Right,
            'a' => Action::Activate,
            'b' => Action::Back,
            'p' => Action::Prev,
            'n' => Action::Next,
            _ => return None,
        })
    }
}

/// A mouse button, for [`Engine::press`]. The primary button is also what [`Pointer::down`] means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Left button or a finger.
    Primary,
    /// Right button: the game's alternative action (for example equip or move an item).
    Secondary,
    /// Middle button.
    Middle,
}

/// Pointer state in device pixels, top-left origin. Use negative coordinates for "no pointer".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    /// Horizontal position in device pixels.
    pub x: f32,
    /// Vertical position in device pixels.
    pub y: f32,
    /// Whether the primary button (or a finger) is down.
    pub down: bool,
}

impl Pointer {
    /// A pointer that is not over the game.
    pub const NONE: Pointer = Pointer {
        x: -1.0,
        y: -1.0,
        down: false,
    };
}

/// The game asks for a text field to be edited. Show a keyboard or text box, then answer with
/// [`Engine::submit_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRequest {
    /// Current content of the field.
    pub text: String,
    /// Maximum length in characters, when the field has one.
    pub max_len: Option<usize>,
    /// The field accepts a number only.
    pub numeric: bool,
}

/// RGBA pixels of a texture, borrowed from the engine until its next call.
#[derive(Clone, Copy, Debug)]
pub struct Texture<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, row by row from the top, straight alpha.
    pub rgba: &'a [u8],
}

/// What a port drives: the whole game seen from the outside.
///
/// Typical frame: [`set_pointer`](Engine::set_pointer) and [`action`](Engine::action) for the input that
/// arrived, [`advance`](Engine::advance) with the elapsed time, [`draw`](Engine::draw) into a
/// [`DrawTarget`], upload [`texture`](Engine::texture)s the draw list mentions, then handle
/// [`take_text_request`](Engine::take_text_request) and [`take_sounds`](Engine::take_sounds).
pub trait Engine {
    /// Resize the output to `width` x `height` device pixels. The game keeps its proportions and
    /// reflows to the extra width.
    fn resize(&mut self, width: u32, height: u32);

    /// Advance the game by `seconds` of real time. Logic runs in whole [`STEP_SECONDS`] steps; a
    /// remainder carries over, and very long gaps (a paused host) are clamped to a quarter of a second.
    fn advance(&mut self, seconds: f32);

    /// Set the pointer (mouse or finger). A change of `down` between calls is a press or a release.
    fn set_pointer(&mut self, pointer: Pointer);

    /// A button other than the primary one went down or up with the pointer at `(x, y)` device pixels. Use this for right and
    /// middle clicks; for the primary button and for moving, use [`set_pointer`](Engine::set_pointer).
    fn press(&mut self, x: f32, y: f32, button: MouseButton, down: bool);

    /// Release everything held: the pointer button, keys, the focus and queued presses. Call it when the host loses focus
    /// (a hidden browser tab, a suspended console) so nothing stays stuck.
    fn reset_input(&mut self);

    /// Apply a navigation command. Ignored while a text field is being edited.
    fn action(&mut self, action: Action);

    /// Scroll the focused list, in game pixels (positive `dy` scrolls down).
    fn scroll(&mut self, dx: f32, dy: f32);

    /// A key went down (`down`, with `repeat` for auto-repeat) or up. `key` is a Unity `KeyCode`, see [`keys`]; the game
    /// reads some letters, digits, arrows and modifiers for its own shortcuts. Ignored while a text field is edited.
    fn key(&mut self, key: i32, down: bool, repeat: bool);

    /// A mouse wheel turned by `notches` (positive = up) with the pointer at `(x, y)` device pixels.
    fn wheel(&mut self, x: f32, y: f32, notches: f32);

    /// Build the frame and give it to `target`. Returns the number of quads.
    fn draw(&mut self, target: &mut dyn DrawTarget) -> usize;

    /// Pixels of texture `id` as named by [`Quad::tex`] (an id above [`TEXTURE_ATLAS`], or the atlas
    /// itself), or `None` if unknown. The atlas changes as text is drawn: upload it again after a frame
    /// whenever [`atlas_changed`](Engine::atlas_changed) says so.
    fn texture(&mut self, id: u32) -> Option<Texture<'_>>;

    /// Whether the atlas pixels changed since the last call. Reading clears the flag.
    fn atlas_changed(&mut self) -> bool;

    /// Release the engine's copy of texture `id` once the port has uploaded it.
    fn release_texture(&mut self, id: u32);

    /// The text field the game wants edited, if a new request is pending. Reading clears the request.
    fn take_text_request(&mut self) -> Option<TextRequest>;

    /// Answer a text request: `Some(text)` to commit the edited content, `None` to cancel.
    fn submit_text(&mut self, text: Option<&str>);

    /// Names of the sound clips the game asked to play since the last call.
    fn take_sounds(&mut self) -> Vec<String>;
}

/// Accumulates real time and yields whole logic steps.
///
/// ```
/// use platform_api::{FixedStep, STEP_SECONDS};
/// let mut clock = FixedStep::default();
/// assert_eq!(clock.advance(STEP_SECONDS * 2.5), 2);
/// assert_eq!(clock.advance(STEP_SECONDS * 0.6), 1);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedStep {
    accumulated: f32,
}

impl FixedStep {
    /// Longest gap taken into account in one call, in seconds.
    pub const MAX_GAP: f32 = 0.25;
    /// Most steps one call returns; the rest is dropped so a slow host cannot spiral.
    pub const MAX_STEPS: u32 = 6;

    /// Add `seconds` and return how many logic steps are due now.
    pub fn advance(&mut self, seconds: f32) -> u32 {
        self.accumulated += seconds.clamp(0.0, Self::MAX_GAP);
        let mut steps = 0;
        while self.accumulated >= STEP_SECONDS && steps < Self::MAX_STEPS {
            self.accumulated -= STEP_SECONDS;
            steps += 1;
        }
        if steps == Self::MAX_STEPS {
            self.accumulated = 0.0;
        }
        steps
    }

    /// Seconds accumulated since the last step, in `0.0..STEP_SECONDS`. Renderers use it to interpolate.
    pub fn remainder(&self) -> f32 {
        self.accumulated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_clip_boundaries_are_preserved() {
        let sample = Quad {
            p: [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            col: [-1.0, 0.5, 1.5, 1.0],
            tex: 3,
            nearest: false,
            clip: [0, 0, 10, 10],
        };
        let clipped = Quad {
            clip: [0, 0, 5, 5],
            ..sample.clone()
        };
        let mut list = TriangleList::default();
        list.draw_quads(&[sample.clone(), sample.clone(), clipped, sample]);
        let ranges: Vec<_> = list.batches.iter().map(|b| (b.first, b.count)).collect();
        assert_eq!(ranges, [(0, 12), (12, 6), (18, 6)]);
        assert_eq!(
            list.vertices[5],
            Vertex {
                x: 0.0,
                y: 1.0,
                u: 0.0,
                v: 1.0,
                rgba: 0xffff8000,
            }
        );
        assert_eq!(std::mem::size_of::<Vertex>(), 20);
        assert_eq!(std::mem::size_of::<Batch>(), 32);
        list.draw_quads(&[]);
        assert!(list.vertices.is_empty() && list.batches.is_empty());
    }

    #[test]
    fn action_codes_round_trip() {
        for action in [
            Action::Up,
            Action::Down,
            Action::Left,
            Action::Right,
            Action::Activate,
            Action::Back,
            Action::Prev,
            Action::Next,
        ] {
            assert_eq!(Action::from_code(action.code()), Some(action));
        }
        assert_eq!(Action::from_code('x'), None);
    }

    #[test]
    fn fixed_step_carries_remainders_and_bounds_long_gaps() {
        let mut clock = FixedStep::default();
        assert_eq!(clock.advance(STEP_SECONDS * 0.9), 0);
        assert_eq!(clock.advance(STEP_SECONDS * 0.2), 1);
        assert!(clock.remainder() < STEP_SECONDS);
        assert_eq!(clock.advance(60.0), FixedStep::MAX_STEPS);
        assert_eq!(clock.remainder(), 0.0);
        assert_eq!(clock.advance(-1.0), 0);
    }
}
