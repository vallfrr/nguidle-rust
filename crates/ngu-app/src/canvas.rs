//! Immediate-mode drawing for the setup window: quads in a 960 x 600 space.

use clr::platform_api::Quad;
use fontdue::Font;
use std::collections::HashMap;

/// Size of the virtual screen, the same as the game's.
pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 600.0;

/// Texture numbers: 0 is white, 1 the glyph atlas, the rest are art.
pub const TEX_WHITE: u32 = 0;
pub const TEX_ATLAS: u32 = 1;
pub const TEX_FRAME: u32 = 2;
pub const TEX_BUTTON: u32 = 3;
pub const TEX_BACKDROP: u32 = 4;
pub const TEX_CAT: u32 = 5;

pub const ATLAS: usize = 1024;

/// Which font to draw with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    Title,
    Body,
}

#[derive(Clone, Copy)]
struct Glyph {
    uv: [f32; 4],
    width: f32,
    height: f32,
    left: f32,
    top: f32,
    advance: f32,
}

/// Rasterised glyphs shelf-packed into one alpha texture (white with coverage as alpha).
pub struct Atlas {
    pub pixels: Vec<u8>,
    pub dirty: bool,
    glyphs: HashMap<(Face, u32, char), Glyph>,
    shelf: (usize, usize, usize),
}

impl Default for Atlas {
    fn default() -> Self {
        Atlas {
            pixels: vec![0; ATLAS * ATLAS * 4],
            dirty: true,
            glyphs: HashMap::new(),
            shelf: (1, 1, 0),
        }
    }
}

impl Atlas {
    fn glyph(&mut self, fonts: &Fonts, face: Face, size: u32, c: char) -> Glyph {
        if let Some(glyph) = self.glyphs.get(&(face, size, c)) {
            return *glyph;
        }
        let (m, coverage) = fonts.get(face).rasterize(c, size as f32);
        let (w, h) = (m.width, m.height);
        let (mut x, mut y, mut row) = self.shelf;
        if x + w + 1 > ATLAS {
            x = 1;
            y += row + 1;
            row = 0;
        }
        let fits = y + h + 1 <= ATLAS;
        let mut uv = [0.0; 4];
        if fits && w > 0 && h > 0 {
            for gy in 0..h {
                for gx in 0..w {
                    let at = ((y + gy) * ATLAS + x + gx) * 4;
                    self.pixels[at..at + 4].copy_from_slice(&[
                        255,
                        255,
                        255,
                        coverage[gy * w + gx],
                    ]);
                }
            }
            let n = ATLAS as f32;
            uv = [
                x as f32 / n,
                y as f32 / n,
                (x + w) as f32 / n,
                (y + h) as f32 / n,
            ];
            self.shelf = (x + w + 1, y, row.max(h));
            self.dirty = true;
        } else {
            self.shelf = (x, y, row);
        }
        let glyph = Glyph {
            uv,
            width: if fits { w as f32 } else { 0.0 },
            height: h as f32,
            left: m.xmin as f32,
            top: m.ymin as f32,
            advance: m.advance_width,
        };
        self.glyphs.insert((face, size, c), glyph);
        glyph
    }
}

/// The two fonts.
pub struct Fonts {
    pub title: Font,
    pub body: Font,
}

impl Fonts {
    fn get(&self, face: Face) -> &Font {
        match face {
            Face::Title => &self.title,
            Face::Body => &self.body,
        }
    }
}

/// A frame of quads being built.
pub struct Canvas<'a> {
    pub quads: Vec<Quad>,
    pub atlas: &'a mut Atlas,
    pub fonts: &'a Fonts,
}

fn quad(rect: [f32; 4], uv: [f32; 4], tex: u32, col: [f32; 4]) -> Quad {
    let [x0, y0, x1, y1] = rect;
    let [u0, v0, u1, v1] = uv;
    Quad {
        p: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        uv: [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
        col,
        tex,
        nearest: false,
        clip: [i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2],
    }
}

impl<'a> Canvas<'a> {
    pub fn new(atlas: &'a mut Atlas, fonts: &'a Fonts) -> Self {
        Canvas {
            quads: Vec::new(),
            atlas,
            fonts,
        }
    }

    /// A flat colour rectangle `[x0, y0, x1, y1]`.
    pub fn fill(&mut self, rect: [f32; 4], col: [f32; 4]) {
        self.quads
            .push(quad(rect, [0.0, 0.0, 1.0, 1.0], TEX_WHITE, col));
    }

    /// A picture stretched over `rect`.
    pub fn image(&mut self, tex: u32, rect: [f32; 4], col: [f32; 4]) {
        self.quads.push(quad(rect, [0.0, 0.0, 1.0, 1.0], tex, col));
    }

    /// A picture whose corners keep their size (`slice` pixels of a `size`-pixel square) and whose
    /// edges and middle stretch.
    pub fn nine(&mut self, tex: u32, size: f32, slice: f32, rect: [f32; 4], col: [f32; 4]) {
        let [x0, y0, x1, y1] = rect;
        let edge = slice.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
        let xs = [x0, x0 + edge, x1 - edge, x1];
        let ys = [y0, y0 + edge, y1 - edge, y1];
        let t = [0.0, slice / size, 1.0 - slice / size, 1.0];
        for j in 0..3 {
            for i in 0..3 {
                self.quads.push(quad(
                    [xs[i], ys[j], xs[i + 1], ys[j + 1]],
                    [t[i], t[j], t[i + 1], t[j + 1]],
                    tex,
                    col,
                ));
            }
        }
    }

    /// Width of `text` on one line.
    pub fn measure(&mut self, face: Face, size: u32, text: &str) -> f32 {
        text.chars()
            .map(|c| self.atlas.glyph(self.fonts, face, size, c).advance)
            .sum()
    }

    /// Lines of at most `width` pixels (words are not split unless one is too long).
    pub fn wrap(&mut self, face: Face, size: u32, text: &str, width: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if line.is_empty() || self.measure(face, size, &candidate) <= width {
                    line = candidate;
                } else {
                    lines.push(std::mem::replace(&mut line, word.to_owned()));
                }
            }
            lines.push(line);
        }
        lines
    }

    /// One line of text with its top-left at `(x, y)`.
    pub fn text(&mut self, face: Face, size: u32, text: &str, x: f32, y: f32, col: [f32; 4]) {
        let ascent = self
            .fonts
            .get(face)
            .horizontal_line_metrics(size as f32)
            .map_or(size as f32, |m| m.ascent);
        let baseline = (y + ascent).round();
        let mut pen = x;
        for c in text.chars() {
            let g = self.atlas.glyph(self.fonts, face, size, c);
            if g.width > 0.0 {
                let (gx, gy) = ((pen + g.left).round(), baseline - g.top - g.height);
                self.quads.push(quad(
                    [gx, gy, gx + g.width, gy + g.height],
                    g.uv,
                    TEX_ATLAS,
                    col,
                ));
            }
            pen += g.advance;
        }
    }

    /// Text centred on `x`.
    pub fn text_centered(
        &mut self,
        face: Face,
        size: u32,
        text: &str,
        x: f32,
        y: f32,
        col: [f32; 4],
    ) {
        let width = self.measure(face, size, text);
        self.text(face, size, text, x - width / 2.0, y, col);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::Art;

    fn parts() -> (Atlas, Fonts) {
        let art = Art::load();
        (
            Atlas::default(),
            Fonts {
                title: art.title,
                body: art.body,
            },
        )
    }

    #[test]
    fn nine_slice_makes_nine_quads_that_cover_the_rectangle() {
        let (mut atlas, fonts) = parts();
        let mut canvas = Canvas::new(&mut atlas, &fonts);
        canvas.nine(TEX_FRAME, 200.0, 24.0, [10.0, 20.0, 110.0, 80.0], [1.0; 4]);
        assert_eq!(canvas.quads.len(), 9);
        let right = canvas.quads.iter().map(|q| q.p[2][0]).fold(0.0, f32::max);
        let bottom = canvas.quads.iter().map(|q| q.p[2][1]).fold(0.0, f32::max);
        assert_eq!((right, bottom), (110.0, 80.0));
    }

    #[test]
    fn wrapping_keeps_lines_within_the_width() {
        let (mut atlas, fonts) = parts();
        let mut canvas = Canvas::new(&mut atlas, &fonts);
        let lines = canvas.wrap(
            Face::Body,
            20,
            "one two three four five six seven eight nine ten",
            200.0,
        );
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(canvas.measure(Face::Body, 20, line) <= 200.0, "{line}");
        }
        assert_eq!(
            lines.join(" "),
            "one two three four five six seven eight nine ten"
        );
    }

    #[test]
    fn text_draws_one_quad_per_visible_glyph_and_marks_the_atlas() {
        let (mut atlas, fonts) = parts();
        let mut canvas = Canvas::new(&mut atlas, &fonts);
        canvas.text(Face::Title, 24, "Play", 0.0, 0.0, [1.0; 4]);
        assert_eq!(canvas.quads.len(), 4);
        assert!(canvas.atlas.dirty);
        assert!(canvas.quads.iter().all(|q| q.tex == TEX_ATLAS));
    }
}
