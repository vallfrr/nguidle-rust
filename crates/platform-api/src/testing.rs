//! A synthetic [`Engine`] for testing a port without the game's files.
//!
//! [`ScriptedEngine`] follows the same contract as the real engine but draws a tiny deterministic
//! scene: a background quad, a cursor quad that follows the pointer, and one square per completed press.
//! A press asks for the sound `click`, [`Action::Activate`] asks for a text field, and the first frame
//! after a resize replaces the atlas. A port's tests can therefore check that it delivers input,
//! steps time, uploads textures and handles text requests, in CI, with nothing to download.

use crate::{
    Action, DrawTarget, Engine, FixedStep, Pointer, Quad, TextRequest, Texture, TEXTURE_ATLAS,
};

/// Id of the one non-atlas texture the scripted scene uses (a 2 x 2 checker).
pub const CHECKER_TEXTURE: u32 = 2;

const CHECKER: [u8; 16] = [
    255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255,
];

/// Deterministic stand-in for the engine. See the [module documentation](self).
pub struct ScriptedEngine {
    size: (u32, u32),
    clock: FixedStep,
    /// Logic steps run so far.
    pub steps: u64,
    /// Pointer as last set.
    pub pointer: Pointer,
    /// Completed presses (down then up) so far.
    pub clicks: u32,
    /// Navigation actions received, in order.
    pub actions: Vec<Action>,
    /// Text submitted with [`Engine::submit_text`]; `None` entries are cancellations.
    pub submitted: Vec<Option<String>>,
    /// Scroll deltas received, in order.
    pub scrolls: Vec<(f32, f32)>,
    text_pending: bool,
    sounds: Vec<String>,
    atlas: Vec<u8>,
    atlas_changed: bool,
    released: Vec<u32>,
}

impl ScriptedEngine {
    /// A scripted engine drawing into a `width` x `height` screen.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            size: (width, height),
            clock: FixedStep::default(),
            steps: 0,
            pointer: Pointer::NONE,
            clicks: 0,
            actions: Vec::new(),
            submitted: Vec::new(),
            scrolls: Vec::new(),
            text_pending: false,
            sounds: Vec::new(),
            atlas: vec![255; 4],
            atlas_changed: true,
            released: Vec::new(),
        }
    }

    /// Textures released with [`Engine::release_texture`], in order.
    pub fn released_textures(&self) -> &[u32] {
        &self.released
    }
}

fn quad(x: f32, y: f32, w: f32, h: f32, tex: u32, col: [f32; 4], clip: [i32; 4]) -> Quad {
    Quad {
        p: [[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
        uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        col,
        tex,
        nearest: tex == CHECKER_TEXTURE,
        clip,
    }
}

impl Engine for ScriptedEngine {
    fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != self.size {
            self.size = (width, height);
            self.atlas = vec![128; 4];
            self.atlas_changed = true;
        }
    }

    fn advance(&mut self, seconds: f32) {
        self.steps += u64::from(self.clock.advance(seconds));
    }

    fn set_pointer(&mut self, pointer: Pointer) {
        if self.pointer.down && !pointer.down {
            self.clicks += 1;
        }
        if !self.pointer.down && pointer.down {
            self.sounds.push("click".into());
        }
        self.pointer = pointer;
    }

    fn action(&mut self, action: Action) {
        if self.text_pending {
            return;
        }
        self.actions.push(action);
        if action == Action::Activate {
            self.text_pending = true;
        }
    }

    fn scroll(&mut self, dx: f32, dy: f32) {
        self.scrolls.push((dx, dy));
    }

    fn draw(&mut self, target: &mut dyn DrawTarget) -> usize {
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let clip = [0, 0, self.size.0 as i32, self.size.1 as i32];
        let mut quads = vec![quad(0.0, 0.0, w, h, 0, [0.2, 0.3, 0.4, 1.0], clip)];
        for index in 0..self.clicks.min(16) {
            quads.push(quad(
                8.0 + index as f32 * 12.0,
                8.0,
                10.0,
                10.0,
                CHECKER_TEXTURE,
                [1.0; 4],
                clip,
            ));
        }
        if self.pointer.x >= 0.0 {
            quads.push(quad(
                self.pointer.x - 4.0,
                self.pointer.y - 4.0,
                8.0,
                8.0,
                0,
                [1.0, 0.9, 0.0, 1.0],
                clip,
            ));
        }
        target.draw_quads(&quads);
        quads.len()
    }

    fn texture(&mut self, id: u32) -> Option<Texture<'_>> {
        match id {
            CHECKER_TEXTURE => Some(Texture {
                width: 2,
                height: 2,
                rgba: &CHECKER,
            }),
            TEXTURE_ATLAS => Some(Texture {
                width: 1,
                height: 1,
                rgba: &self.atlas,
            }),
            _ => None,
        }
    }

    fn atlas_changed(&mut self) -> bool {
        std::mem::take(&mut self.atlas_changed)
    }

    fn release_texture(&mut self, id: u32) {
        self.released.push(id);
    }

    fn take_text_request(&mut self) -> Option<TextRequest> {
        self.text_pending.then(|| TextRequest {
            text: "Bob".into(),
            max_len: Some(12),
            numeric: false,
        })
    }

    fn submit_text(&mut self, text: Option<&str>) {
        if std::mem::take(&mut self.text_pending) {
            self.submitted.push(text.map(str::to_owned));
        }
    }

    fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TriangleList, STEP_SECONDS};

    #[test]
    fn a_press_draws_a_square_and_asks_for_a_sound() {
        let mut engine = ScriptedEngine::new(320, 200);
        let at = |down| Pointer {
            x: 40.0,
            y: 30.0,
            down,
        };
        engine.set_pointer(at(true));
        assert_eq!(engine.take_sounds(), ["click"]);
        engine.set_pointer(at(false));
        let mut list = TriangleList::default();
        assert_eq!(engine.draw(&mut list), 3);
        assert_eq!(list.vertices.len(), 18);
        assert_eq!(list.batches.len(), 3);
        assert!(engine.texture(CHECKER_TEXTURE).is_some());
        assert!(engine.texture(99).is_none());
    }

    #[test]
    fn text_requests_are_answered_once() {
        let mut engine = ScriptedEngine::new(320, 200);
        engine.action(Action::Activate);
        let request = engine.take_text_request().unwrap();
        assert_eq!((request.text.as_str(), request.max_len), ("Bob", Some(12)));
        engine.action(Action::Down); // ignored while editing
        engine.submit_text(Some("Alice"));
        engine.submit_text(Some("late")); // nothing pending any more
        assert!(engine.take_text_request().is_none());
        assert_eq!(engine.submitted, [Some("Alice".to_string())]);
        assert_eq!(engine.actions, [Action::Activate]);
    }

    #[test]
    fn time_runs_in_whole_steps_and_resizing_replaces_the_atlas() {
        let mut engine = ScriptedEngine::new(320, 200);
        engine.advance(STEP_SECONDS * 3.2);
        assert_eq!(engine.steps, 3);
        assert!(engine.atlas_changed());
        assert!(!engine.atlas_changed());
        engine.resize(640, 400);
        assert!(engine.atlas_changed());
    }
}
