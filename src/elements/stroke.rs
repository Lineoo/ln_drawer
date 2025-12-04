use hashbrown::HashMap;
use palette::Srgb;

use crate::{
    elements::Menu,
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
    pub color: Srgb<u8>,
}
impl Element for StrokeLayer {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider::fullscreen(ZOrder::new(-100)));

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Moved(position) | PointerEvent::Pressed(position) => {
                let mut stroke = world.fetch_mut(this).unwrap();
                stroke.draw(position, world);
            }
            PointerEvent::RightClick(position) => {
                world.build(Menu::test_descriptor(position));
            }
            _ => (),
        });
    }
}
impl StrokeLayer {
    pub fn draw(&mut self, point: Position, world: &World) {
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

        chunk.painter.set_pixel(
            point,
            [self.color.red, self.color.green, self.color.blue, 255],
        );
    }
}

struct StrokeChunk {
    painter: Painter,
}
