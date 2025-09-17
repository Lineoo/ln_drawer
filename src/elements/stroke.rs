use hashbrown::HashMap;

use crate::{
    elements::{Element, Palette, intersect::IntersectFail},
    interface::{Interface, Painter},
    lnwin::PointerEvent,
    world::{ElementHandle, WorldCell},
};

const CHUNK_SIZE: i32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<[i32; 2], StrokeChunk>,
}
impl Element for StrokeLayer {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.observe::<IntersectFail>(move |event, world| match event.0 {
            PointerEvent::Moved(position) | PointerEvent::Pressed(position) => {
                let mut stroke = world.fetch_mut::<StrokeLayer>(handle).unwrap();
                let color = (world.single::<Palette>())
                    .map(|palette| palette.pick_color())
                    .unwrap_or([0xff; 4]);
                stroke.write_pixel(position.into_array(), color, world);
            }
            _ => (),
        });
    }
}
impl StrokeLayer {
    pub fn write_pixel(&mut self, point: [i32; 2], color: [u8; 4], world: &WorldCell) {
        let mut interface = world.single_mut::<Interface>().unwrap();
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
            StrokeChunk { painter }
        });

        chunk.painter.set_pixel(point[0], point[1], color);
    }
}

struct StrokeChunk {
    painter: Painter,
}
