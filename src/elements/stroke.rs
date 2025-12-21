use hashbrown::HashMap;
use palette::Srgba;

use crate::{
    elements::{menu::Menu, palette::Palette},
    lnwin::{LnwinModifiers, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    render::canvas::{Canvas, CanvasDescriptor},
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Descriptor, Element, Handle, World},
};

const CHUNK_SIZE: i32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<(i32, i32), StrokeChunk>,
    pub color: Srgba,
}

pub struct StrokeChunk {
    canvas: Canvas,
}

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct StrokeLayerDescriptor {
    pub chunks: Vec<StrokeChunkDescriptor>,
    pub color: (f32, f32, f32, f32),
}

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct StrokeChunkDescriptor {
    pub key: (i32, i32),
    pub data: Option<Vec<u8>>,
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

impl Descriptor for StrokeLayerDescriptor {
    type Target = StrokeLayer;

    fn build(self, world: &World) -> Self::Target {
        let mut layer = StrokeLayer {
            chunks: HashMap::new(),
            color: Srgba::from_components(self.color),
        };

        for chunk in self.chunks {
            layer.chunks.insert(chunk.key, world.build(chunk));
        }

        layer
    }
}

impl Descriptor for StrokeChunkDescriptor {
    type Target = StrokeChunk;

    fn build(self, world: &World) -> Self::Target {
        StrokeChunk {
            canvas: world.build(CanvasDescriptor {
                rect: Rectangle {
                    origin: Position::new(self.key.0 * CHUNK_SIZE, self.key.1 * CHUNK_SIZE),
                    extend: Delta::splat(CHUNK_SIZE),
                },
                order: 0,
                visible: true,
                data: self.data,
                width: CHUNK_SIZE as u32,
                height: CHUNK_SIZE as u32,
            }),
        }
    }
}

impl StrokeLayer {
    pub fn to_descriptor(&self) -> StrokeLayerDescriptor {
        let mut layer = StrokeLayerDescriptor::default();

        for (key, chunk) in &self.chunks {
            let painter = chunk.canvas.to_descriptor();
            layer.chunks.push(StrokeChunkDescriptor {
                key: *key,
                data: painter.data,
            });
        }

        layer.color = self.color.into_components();

        layer
    }

    pub fn draw(&mut self, point: Position, world: &World) {
        let chunk_key = (
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        );

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| {
            world.build(StrokeChunkDescriptor {
                key: chunk_key,
                data: None,
            })
        });

        let (wx, wy) = StrokeLayer::world_to_texture(point, chunk.canvas.rect);
        chunk.canvas.draw(wx, wy, self.color);
    }

    pub fn pick(&mut self, point: Position, world: &World) {
        let chunk_key = (
            point.x.div_euclid(CHUNK_SIZE),
            point.y.div_euclid(CHUNK_SIZE),
        );

        if let Some(chunk) = self.chunks.get(&chunk_key) {
            let (wx, wy) = StrokeLayer::world_to_texture(point, chunk.canvas.rect);
            self.color = chunk.canvas.read(wx, wy);

            world.foreach_fetch_mut::<Palette>(|_, mut palette| {
                palette.set_color(self.color);
            });
        }
    }

    fn world_to_texture(point: Position, rect: Rectangle) -> (i32, i32) {
        let relative_x = point.x - rect.origin.x;
        let relative_y = point.y - rect.origin.y;

        let width = rect.width();
        let height = rect.height();

        let wrapped_x = (relative_x).rem_euclid(width as i32);
        let wrapped_y = (height as i32 - 1 - relative_y).rem_euclid(height as i32);

        (wrapped_x, wrapped_y)
    }
}
