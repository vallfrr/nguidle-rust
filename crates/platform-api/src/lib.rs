//! Platform-independent draw data and conversion for rendering hosts.

#[derive(Clone)]
pub struct Quad {
    /// Screen-space corners in origin, +u, +u+v, +v order.
    pub p: [[f32; 2]; 4],
    /// Texture coordinates with V increasing downwards, in the same order.
    pub uv: [[f32; 2]; 4],
    pub col: [f32; 4],
    pub tex: u32,
    pub nearest: bool,
    pub clip: [i32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    /// RGBA bytes ordered from the least significant to the most significant.
    pub rgba: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch {
    pub tex: u32,
    pub nearest: u32,
    pub clip: [i32; 4],
    pub first: u32,
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
    fn draw_quads(&mut self, quads: &[Quad]);
}

/// Reusable triangle buffers for renderers that batch textured geometry.
#[derive(Default)]
pub struct TriangleList {
    pub vertices: Vec<Vertex>,
    pub batches: Vec<Batch>,
}

impl DrawTarget for TriangleList {
    fn draw_quads(&mut self, quads: &[Quad]) {
        tessellate(quads, &mut self.vertices, &mut self.batches);
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
}
