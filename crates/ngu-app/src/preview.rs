//! Draws the setup window's quads on the CPU, to look at each screen as a picture
//! and to test the layout without a window.

use crate::art::{Art, Image};
use crate::canvas::*;
use crate::setup::Model;
use clr::platform_api::Quad;

fn sample(image: &Image, u: f32, v: f32) -> [f32; 4] {
    let x = ((u * image.width as f32) as i64).clamp(0, image.width as i64 - 1) as usize;
    let y = ((v * image.height as f32) as i64).clamp(0, image.height as i64 - 1) as usize;
    let at = (y * image.width as usize + x) * 4;
    let p = &image.rgba[at..at + 4];
    [
        f32::from(p[0]) / 255.0,
        f32::from(p[1]) / 255.0,
        f32::from(p[2]) / 255.0,
        f32::from(p[3]) / 255.0,
    ]
}

/// Rasterises axis-aligned quads (all the setup window uses) into a 960 x 600 picture.
pub fn raster(quads: &[Quad], art: &Art, atlas: &Atlas) -> Image {
    let white = Image {
        width: 1,
        height: 1,
        rgba: vec![255; 4],
    };
    let glyphs = Image {
        width: ATLAS as u32,
        height: ATLAS as u32,
        rgba: atlas.pixels.clone(),
    };
    let mut out = vec![0u8; (WIDTH * HEIGHT) as usize * 4];
    for q in quads {
        let image = match q.tex {
            TEX_ATLAS => &glyphs,
            TEX_FRAME => &art.frame,
            TEX_BUTTON => &art.button,
            TEX_BACKDROP => &art.backdrop,
            TEX_CAT => &art.cat,
            _ => &white,
        };
        let (x0, y0, x1, y1) = (q.p[0][0], q.p[0][1], q.p[2][0], q.p[2][1]);
        for y in (y0.floor().max(0.0) as usize)..(y1.ceil().min(HEIGHT) as usize) {
            for x in (x0.floor().max(0.0) as usize)..(x1.ceil().min(WIDTH) as usize) {
                let (tx, ty) = (
                    (x as f32 + 0.5 - x0) / (x1 - x0),
                    (y as f32 + 0.5 - y0) / (y1 - y0),
                );
                let u = q.uv[0][0] + (q.uv[2][0] - q.uv[0][0]) * tx;
                let v = q.uv[0][1] + (q.uv[2][1] - q.uv[0][1]) * ty;
                let s = sample(image, u, v);
                let alpha = s[3] * q.col[3];
                let at = (y * WIDTH as usize + x) * 4;
                for c in 0..3 {
                    let dst = f32::from(out[at + c]) / 255.0;
                    out[at + c] =
                        ((dst * (1.0 - alpha) + s[c] * q.col[c] * alpha) * 255.0 + 0.5) as u8;
                }
                out[at + 3] = 255;
            }
        }
    }
    Image {
        width: WIDTH as u32,
        height: HEIGHT as u32,
        rgba: out,
    }
}

/// The current screen of `model` as a picture.
pub fn screen(model: &Model, art: &Art, time: f32) -> Image {
    let mut atlas = Atlas::default();
    let fonts = Fonts {
        title: Art::load().title,
        body: Art::load().body,
    };
    let mut canvas = Canvas::new(&mut atlas, &fonts);
    model.view(&mut canvas, (-1.0, -1.0), time);
    let quads = std::mem::take(&mut canvas.quads);
    raster(&quads, art, &atlas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_screen_shows_text_and_a_button_over_the_backdrop() {
        let dir = tempfile::tempdir().unwrap();
        let model = Model::new(dir.path().join("app"), String::new());
        let picture = screen(&model, &Art::load(), 0.0);
        assert_eq!(picture.rgba.len(), 960 * 600 * 4);
        let distinct: std::collections::HashSet<_> =
            picture.rgba.chunks(4).map(|p| (p[0], p[1], p[2])).collect();
        assert!(distinct.len() > 50, "{}", distinct.len());
    }
}
