use hashbrown::HashMap;

use crate::{
    elements::Element,
    interface::{Interface, Painter, Wireframe},
};

const CHUNK_SIZE: i32 = 512;

pub struct StrokeLayer {
    chunks: HashMap<[i32; 2], StrokeChunk>,
}
impl Element for StrokeLayer {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "stroke".into()
    }
    // TODO: Use Optional Border
    fn border(&self) -> [i32; 4] {
        [0; 4]
    }
}
impl StrokeLayer {
    pub fn new() -> StrokeLayer {
        StrokeLayer {
            chunks: HashMap::new(),
        }
    }
    pub fn write_pixel(&mut self, point: [i32; 2], color: [u8; 4], interface: &mut Interface) {
        let chunk_key = [
            point[0].div_euclid(CHUNK_SIZE),
            point[1].div_euclid(CHUNK_SIZE),
        ];
        let chunk_orig = [chunk_key[0] * CHUNK_SIZE, chunk_key[1] * CHUNK_SIZE];

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| {
            let painter = interface.create_painter([
                chunk_orig[0],
                chunk_orig[1],
                chunk_orig[0] + CHUNK_SIZE,
                chunk_orig[1] + CHUNK_SIZE,
            ]);
            let debug_wireframe = interface.create_wireframe(
                [
                    chunk_orig[0],
                    chunk_orig[1],
                    chunk_orig[0] + CHUNK_SIZE,
                    chunk_orig[1] + CHUNK_SIZE,
                ],
                [1.0, 0.5, 0.5, 1.0],
            );
            StrokeChunk {
                painter,
                debug_wireframe,
            }
        });

        chunk.painter.set_pixel(point[0], point[1], color);
    }
}

struct StrokeChunk {
    painter: Painter,
    debug_wireframe: Wireframe,
}
