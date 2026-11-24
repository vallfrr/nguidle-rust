//! The drawing side of the port. PORT: replace the body of [`Display::draw_quads`] and
//! [`Display::upload_missing`] with calls to your graphics API.

use clr::platform_api::{DrawTarget, Engine, Quad, TriangleList, TEXTURE_ATLAS};
use std::collections::HashSet;

/// Receives each frame and keeps track of the textures it has seen.
#[derive(Default)]
pub struct Display {
    /// The last frame as vertex and batch arrays, the layout most graphics APIs want.
    pub frame: TriangleList,
    uploaded: HashSet<u32>,
    /// Textures uploaded so far.
    pub uploads: usize,
}

impl DrawTarget for Display {
    fn draw_quads(&mut self, quads: &[Quad]) {
        // PORT: draw `quads` in order, each over the previous ones, clipped to `quad.clip`.
        // Here they are only converted; a real port submits `self.frame.vertices` by `self.frame.batches`.
        self.frame.draw_quads(quads);
    }
}

impl Display {
    /// Upload every texture the last frame uses and has not been uploaded yet, and the glyph atlas when it changed.
    pub fn upload_missing<E: Engine>(&mut self, engine: &mut E) {
        let wanted: Vec<u32> = self
            .frame
            .batches
            .iter()
            .map(|batch| batch.tex)
            .filter(|id| *id > TEXTURE_ATLAS)
            .collect();
        for id in wanted {
            if self.uploaded.insert(id) {
                // PORT: create a GPU texture from `texture.rgba` (width x height, RGBA8, top row first).
                if engine.texture(id).is_some() {
                    self.uploads += 1;
                }
                engine.release_texture(id);
            }
        }
        if engine.atlas_changed() {
            // PORT: upload the atlas again: `engine.texture(TEXTURE_ATLAS)`.
            let _ = engine.texture(TEXTURE_ATLAS);
        }
    }
}
