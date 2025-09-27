use hashbrown::HashMap;

use crate::{
    elements::{Element, Palette, tools::pointer::PointerHit},
    interface::{Interface, Painter},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle},
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
        this.observe::<PointerHit>(move |event, world| match event.0 {
            PointerEvent::Moved(position) | PointerEvent::Pressed(position) => {
                let mut stroke = world.fetch_mut::<StrokeLayer>(handle).unwrap();
                let color = (world.single::<Palette>())
                    .map(|palette| palette.pick_color())
                    .unwrap_or([0xff; 4]);
                stroke.write_pixel(position, color, world);
            }
            _ => (),
        });
    }
}
impl StrokeLayer {
    pub fn write_pixel(&mut self, point: Position, color: [u8; 4], world: &WorldCell) {
        let mut interface = world.single_mut::<Interface>().unwrap();
        let chunk_key = [
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        ];
        let chunk_orig = Position::new(chunk_key[0] * CHUNK_SIZE, chunk_key[1] * CHUNK_SIZE);

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| {
            let painter = interface.create_painter(Rectangle {
                origin: chunk_orig,
                extend: Delta::new(CHUNK_SIZE, CHUNK_SIZE),
            });
            StrokeChunk { painter }
        });

        chunk.painter.set_pixel(point, color);
    }
}

struct StrokeChunk {
    painter: Painter,
}
