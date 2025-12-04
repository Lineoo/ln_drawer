use hashbrown::HashMap;

use crate::{
    elements::Palette,
    interface::{Interface, Painter},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, Handle, World},
};

const CHUNK_SIZE: i32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<[i32; 2], StrokeChunk>,
}
impl Element for StrokeLayer {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: Rectangle {
                origin: Position::splat(i32::MIN / 2),
                extend: Delta::splat(i32::MAX),
            },
            z_order: ZOrder::new(-100),
        });

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Moved(position) | PointerEvent::Pressed(position) => {
                let mut stroke = world.fetch_mut(this).unwrap();
                let color = (world.single_fetch::<Palette>())
                    .map(|palette| palette.pick_color())
                    .unwrap_or([0xff; 4]);
                stroke.write_pixel(position, color, world);
            }
            _ => (),
        });

    }
}
impl StrokeLayer {
    pub fn write_pixel(&mut self, point: Position, color: [u8; 4], world: &World) {
        let mut interface = world.single_fetch_mut::<Interface>().unwrap();
        let chunk_key = [
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        ];
        let chunk_orig = Position::new(chunk_key[0] * CHUNK_SIZE, chunk_key[1] * CHUNK_SIZE);

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| StrokeChunk {
            painter: Painter::new(
                Rectangle {
                    origin: chunk_orig,
                    extend: Delta::new(CHUNK_SIZE, CHUNK_SIZE),
                },
                &mut interface,
            ),
        });

        chunk.painter.set_pixel(point, color);
    }
}

struct StrokeChunk {
    painter: Painter,
}
