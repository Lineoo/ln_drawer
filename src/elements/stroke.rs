use hashbrown::HashMap;

use crate::{
    elements::Element,
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
        let mut cursor_down = false;
        let mut stroke = world.entry::<StrokeLayer>(handle).unwrap();
        stroke.observe::<PointerEvent>(move |&event, world| match event {
            PointerEvent::Moved(position) if cursor_down => {
                let mut stroke = world.fetch_mut::<StrokeLayer>(handle).unwrap();
                stroke.write_pixel(position, [0xff; 4], world);
            }
            PointerEvent::Moved(_) => {}
            PointerEvent::Pressed(position) => {
                cursor_down = true;
                let mut stroke = world.fetch_mut::<StrokeLayer>(handle).unwrap();
                stroke.write_pixel(position, [0xff; 4], world);
            }
            PointerEvent::Released(_) => {
                cursor_down = false;
            }
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
