use hashbrown::HashMap;
use palette::Srgb;

use crate::{
    elements::{menu::Menu, palette::Palette},
    interface::{Interface, Painter},
    lnwin::{LnwinModifiers, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Element, Handle, World},
};

const CHUNK_SIZE: i32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<[i32; 2], StrokeChunk>,
    pub color: Srgb<u8>,
}

struct StrokeChunk {
    painter: Painter,
}

impl Element for StrokeLayer {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider::fullscreen(ZOrder::new(-100)));

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Moved(position) | PointerEvent::Pressed(position) => {
                let mut stroke = world.fetch_mut(this).unwrap();

                let modifiers = world.single_fetch::<LnwinModifiers>().unwrap();
                if modifiers.0.state().alt_key() {
                    stroke.pick(position, world);
                } else {
                    stroke.draw(position, world);
                }
            }
            _ => (),
        });

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.build(Menu::test_descriptor(position));
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

        chunk.painter.set_z_order(ZOrder::new(-100));

        chunk.painter.set_pixel(
            point,
            [self.color.red, self.color.green, self.color.blue, 255],
        );
    }

    pub fn pick(&mut self, point: Position, world: &World) {
        let chunk_key = [
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        ];

        if let Some(chunk) = self.chunks.get(&chunk_key) {
            let color = chunk.painter.get_pixel(point);
            self.color = Srgb::new(color[0], color[1], color[2]);

            world.foreach_fetch_mut::<Palette>(|_, mut palette| {
                palette.set_color(self.color);
            });
        }
    }
}
