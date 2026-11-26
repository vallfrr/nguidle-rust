//! Draws a quad list with miniquad, scaled into the window like the game's own view.

use crate::art::{Art, Image};
use crate::canvas::{Atlas, ATLAS, HEIGHT, WIDTH};
use clr::platform_api::Quad;
use miniquad::*;

const VERTEX: &str = r#"#version 100
attribute vec2 pos;
attribute vec2 uv;
attribute vec4 color;
uniform vec2 screen;
varying highp vec2 vuv;
varying lowp vec4 vcolor;
void main() {
    gl_Position = vec4(pos.x / screen.x * 2.0 - 1.0, 1.0 - pos.y / screen.y * 2.0, 0.0, 1.0);
    vuv = uv;
    vcolor = color / 255.0;
}"#;

const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying highp vec2 vuv;
varying lowp vec4 vcolor;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, vuv) * vcolor;
}"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vert {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
struct Uniforms {
    screen: (f32, f32),
}

const MAX_QUADS: usize = 4000;

/// The graphics objects of the setup window.
pub struct Gpu {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    vbuf: BufferId,
    ibuf: BufferId,
    /// Indexed by texture number (see `canvas`).
    textures: Vec<TextureId>,
}

/// Window position and scale of the 960 x 600 view, letterboxed.
pub fn view(w: f32, h: f32) -> (f32, f32, f32) {
    let s = (w / WIDTH).min(h / HEIGHT);
    (s, (w - WIDTH * s) / 2.0, (h - HEIGHT * s) / 2.0)
}

impl Gpu {
    pub fn new(art: &Art) -> Gpu {
        let mut ctx = window::new_rendering_backend();
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                ShaderMeta {
                    images: vec!["tex".to_string()],
                    uniforms: UniformBlockLayout {
                        uniforms: vec![UniformDesc::new("screen", UniformType::Float2)],
                    },
                },
            )
            .expect("shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
                VertexAttribute::new("color", VertexFormat::Byte4),
            ],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );
        let vbuf = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<Vert>(MAX_QUADS * 4),
        );
        let mut idx: Vec<u16> = Vec::with_capacity(MAX_QUADS * 6);
        for q in 0..MAX_QUADS as u32 {
            idx.extend([0u32, 1, 2, 0, 2, 3].map(|i| (q * 4 + i) as u16));
        }
        let ibuf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&idx),
        );
        let mut upload = |image: &Image| {
            let id =
                ctx.new_texture_from_rgba8(image.width as u16, image.height as u16, &image.rgba);
            ctx.texture_set_wrap(id, TextureWrap::Clamp, TextureWrap::Clamp);
            ctx.texture_set_filter(id, FilterMode::Linear, MipmapFilterMode::None);
            id
        };
        let white = upload(&Image {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        });
        let atlas = upload(&Image {
            width: ATLAS as u32,
            height: ATLAS as u32,
            rgba: vec![0; ATLAS * ATLAS * 4],
        });
        let textures = vec![
            white,
            atlas,
            upload(&art.frame),
            upload(&art.button),
            upload(&art.backdrop),
            upload(&art.cat),
        ];
        Gpu {
            ctx,
            pipeline,
            vbuf,
            ibuf,
            textures,
        }
    }

    /// Draw one frame. `quads` are in the 960 x 600 view.
    pub fn draw(&mut self, quads: &[Quad], atlas: &mut Atlas) {
        let (w, h) = window::screen_size();
        if w < 1.0 || h < 1.0 {
            return;
        }
        if atlas.dirty {
            self.ctx.texture_update(self.textures[1], &atlas.pixels);
            atlas.dirty = false;
        }
        let (s, ox, oy) = view(w, h);
        let mut verts: Vec<Vert> = Vec::with_capacity(quads.len() * 4);
        for q in quads.iter().take(MAX_QUADS) {
            let c = q.col.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            for i in 0..4 {
                verts.push(Vert {
                    pos: [q.p[i][0] * s + ox, q.p[i][1] * s + oy],
                    uv: q.uv[i],
                    color: c,
                });
            }
        }
        self.ctx
            .buffer_update(self.vbuf, BufferSource::slice(&verts));
        self.ctx
            .begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_pipeline(&self.pipeline);
        let mut i = 0;
        let count = quads.len().min(MAX_QUADS);
        while i < count {
            let tex = quads[i].tex;
            let mut j = i + 1;
            while j < count && quads[j].tex == tex {
                j += 1;
            }
            let id = self
                .textures
                .get(tex as usize)
                .copied()
                .unwrap_or(self.textures[0]);
            self.ctx.apply_bindings(&Bindings {
                vertex_buffers: vec![self.vbuf],
                index_buffer: self.ibuf,
                images: vec![id],
            });
            self.ctx
                .apply_uniforms(UniformsSource::table(&Uniforms { screen: (w, h) }));
            self.ctx.draw((i * 6) as i32, ((j - i) * 6) as i32, 1);
            i = j;
        }
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_is_centred_and_keeps_proportions() {
        assert_eq!(view(960.0, 600.0), (1.0, 0.0, 0.0));
        let (s, ox, oy) = view(1920.0, 1000.0);
        assert!((s - 1000.0 / 600.0).abs() < 1e-4);
        assert!(ox > 0.0 && oy.abs() < 1e-3);
    }
}
