use hashbrown::HashMap;
use palette::Srgb;

use crate::{
    elements::{menu::Menu, palette::Palette},
    interface::{Interface, Painter, PainterDescriptor},
    lnwin::{LnwinModifiers, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Element, ElementDescriptor, Handle, World},
};

const CHUNK_SIZE: i32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<(i32, i32), StrokeChunk>,
    pub color: Srgb<u8>,
}

pub struct StrokeChunk {
    painter: Painter,
}

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct StrokeLayerDescriptor {
    pub chunks: Vec<StrokeChunkDescriptor>,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct StrokeChunkDescriptor {
    pub key: (i32, i32),
    pub data: Vec<u8>,
}

impl Element for StrokeLayer {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        world.foreach::<StrokeLayer>(|stroke| {
            // need to keep it singleton
            if stroke != this {
                world.remove(stroke);
            }
        });

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
            world.insert(world.build(Menu::test_descriptor(position)));
        });
    }
}

impl ElementDescriptor for StrokeLayerDescriptor {
    type Target = StrokeLayer;

    fn build(self, world: &World) -> Self::Target {
        StrokeLayer::new(self, &mut world.single_fetch_mut().unwrap())
    }
}

impl StrokeLayer {
    pub fn new(descriptor: StrokeLayerDescriptor, interface: &mut Interface) -> StrokeLayer {
        let mut layer = StrokeLayer {
            chunks: HashMap::new(),
            color: Srgb::from_components(descriptor.color),
        };

        for chunk in descriptor.chunks {
            layer.create_chunk(chunk, interface);
        }

        layer
    }

    pub fn to_descriptor(&self) -> StrokeLayerDescriptor {
        let mut layer = StrokeLayerDescriptor::default();

        for (key, chunk) in &self.chunks {
            let painter = chunk.painter.to_descriptor();
            layer.chunks.push(StrokeChunkDescriptor {
                key: *key,
                data: painter.data,
            });
        }

        layer.color = self.color.into_components();

        layer
    }

    pub fn draw(&mut self, point: Position, world: &World) {
        let mut interface = world.single_fetch_mut::<Interface>().unwrap();
        let chunk_key = (
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        );

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| StrokeChunk {
            painter: Painter::new_empty(
                Rectangle {
                    origin: Position::new(chunk_key.0 * CHUNK_SIZE, chunk_key.1 * CHUNK_SIZE),
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
        let chunk_key = (
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        );

        if let Some(chunk) = self.chunks.get(&chunk_key) {
            let color = chunk.painter.get_pixel(point);
            self.color = Srgb::new(color[0], color[1], color[2]);

            world.foreach_fetch_mut::<Palette>(|_, mut palette| {
                palette.set_color(self.color);
            });
        }
    }

    fn create_chunk(&mut self, descriptor: StrokeChunkDescriptor, interface: &mut Interface) {
        self.chunks.insert(
            descriptor.key,
            StrokeChunk {
                painter: Painter::new(
                    PainterDescriptor {
                        rect: Rectangle {
                            origin: Position::new(
                                descriptor.key.0 * CHUNK_SIZE,
                                descriptor.key.1 * CHUNK_SIZE,
                            ),
                            extend: Delta::splat(CHUNK_SIZE),
                        },
                        z_order: ZOrder::new(0),
                        width: CHUNK_SIZE as u32,
                        height: CHUNK_SIZE as u32,
                        data: descriptor.data,
                    },
                    interface,
                ),
            },
        );
    }
}
