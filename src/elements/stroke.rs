use hashbrown::HashMap;
use palette::Srgba;

use crate::{
    elements::{menu::Menu, palette::Palette},
    measures::{Position, Rectangle, Size},
    render::canvas::{Canvas, CanvasDescriptor},
    theme::{Attach, Luni},
    tools::{
        modifiers::ModifiersTool,
        pointer::{PointerCollider, PointerHit, PointerMenu, PointerStatus},
    },
    widgets::{
        button::{Button, ButtonDescriptor},
        check_button::{CheckButton, CheckButtonDescriptor},
        events::Click,
    },
    world::{Descriptor, Element, Handle, World},
};

const CHUNK_SIZE: u32 = 512;

#[derive(Default)]
pub struct StrokeLayer {
    chunks: HashMap<(i32, i32), StrokeChunk>,
    pub color: Srgba,
}

pub struct StrokeChunk {
    canvas: Handle<Canvas>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StrokeLayerDescriptor {
    pub chunks: Vec<StrokeChunkDescriptor>,
    pub color: (f32, f32, f32, f32),
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StrokeChunkDescriptor {
    pub key: (i32, i32),
    pub data: Option<Vec<u8>>,
}

impl Element for StrokeLayer {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.foreach::<StrokeLayer>(|stroke| {
            // need to keep it singleton
            if stroke != this {
                world.remove(stroke);
            }
        });

        let collider = world.insert(PointerCollider::fullscreen(-100));

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world, _| {
            if let PointerStatus::Moving | PointerStatus::Press = event.status {
                let mut stroke = world.fetch_mut(this).unwrap();

                let tool = world.single_fetch::<ModifiersTool>().unwrap();
                if tool.modifiers.state().alt_key() {
                    stroke.pick(event.position, world);
                } else {
                    stroke.draw(event.position, world);
                }
            }
        });

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.build(Menu::test_descriptor(position));
        });
    }
}

impl Descriptor for StrokeLayerDescriptor {
    type Target = Handle<StrokeLayer>;

    fn when_build(self, world: &World) -> Self::Target {
        let mut layer = StrokeLayer {
            chunks: HashMap::new(),
            color: Srgba::from_components(self.color),
        };

        for chunk in self.chunks {
            layer.chunks.insert(chunk.key, world.build(chunk));
        }

        world.insert(layer)
    }
}

impl Descriptor for StrokeChunkDescriptor {
    type Target = StrokeChunk;

    fn when_build(self, world: &World) -> Self::Target {
        StrokeChunk {
            canvas: world.build(CanvasDescriptor {
                rect: Rectangle {
                    origin: Position::new(
                        self.key.0 * CHUNK_SIZE as i32,
                        self.key.1 * CHUNK_SIZE as i32,
                    ),
                    extend: Size::splat(CHUNK_SIZE),
                },
                order: 0,
                visible: true,
                data: self.data,
                width: CHUNK_SIZE,
                height: CHUNK_SIZE,
            }),
        }
    }
}

impl StrokeLayer {
    pub fn to_descriptor(&self, world: &World) -> StrokeLayerDescriptor {
        let mut layer = StrokeLayerDescriptor::default();

        for (key, chunk) in &self.chunks {
            let canvas = world.fetch(chunk.canvas).unwrap();
            let painter = canvas.to_descriptor();
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
            point.x.div_euclid(CHUNK_SIZE as i32),
            point.y.div_euclid(CHUNK_SIZE as i32),
        );

        let chunk = self.chunks.entry(chunk_key).or_insert_with(|| {
            world.build(StrokeChunkDescriptor {
                key: chunk_key,
                data: None,
            })
        });

        let canvas = chunk.canvas;
        let color = self.color;
        world.queue(move |world| {
            let mut canvas = world.fetch_mut(canvas).unwrap();
            let (wx, wy) = StrokeLayer::world_to_texture(point, canvas.rect);
            canvas.draw(wx, wy, color);
        });
    }

    pub fn pick(&mut self, point: Position, world: &World) {
        let chunk_key = (
            point.x.div_euclid(CHUNK_SIZE as i32),
            point.y.div_euclid(CHUNK_SIZE as i32),
        );

        if let Some(chunk) = self.chunks.get(&chunk_key) {
            let canvas = world.fetch(chunk.canvas).unwrap();
            let (wx, wy) = StrokeLayer::world_to_texture(point, canvas.rect);
            self.color = canvas.read(wx, wy);

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

pub struct StrokeToolbox {
    button: Handle<CheckButton>,
}

pub struct StrokeToolboxDescriptor {
    pub position: Position,
}

impl Descriptor for StrokeToolboxDescriptor {
    type Target = Handle<StrokeToolbox>;

    fn when_build(self, world: &World) -> Self::Target {
        let rect = Rectangle {
            origin: self.position,
            extend: Size::splat(70),
        };

        let button = world.build(CheckButtonDescriptor {
            rect,
            checked: false,
            order: 20,
        });

        world.queue(move |world| {
            let luni = world.single::<Luni>().unwrap();
            world.trigger(luni, &Attach(button));
        });

        world.observer(button, |Click, world, button| {});

        world.insert(StrokeToolbox { button })
    }
}

impl Element for StrokeToolbox {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(self.button, this);
    }
}
