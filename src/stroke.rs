pub mod round_brush;

use cosmic_text::Metrics;
use hashbrown::HashMap;
use palette::Srgba;
use wgpu::Color;
use winit::event::PointerKind;

use crate::{
    animation::{AnimationDescriptor, OnceAnimationDescriptor},
    elements::{
        noise::SimpleNoiseDescriptor,
        palette::{Palette, PaletteDescriptor},
    },
    lnwin::Lnwindow,
    measures::{Position, Rectangle, Size},
    render::{
        Render,
        canvas::{Canvas, CanvasDescriptor},
        text::TextDescriptor,
    },
    stroke::round_brush::RoundBrush,
    tools::{
        modifiers::ModifiersTool,
        mouse::PointerMenu,
        pointer::{PointerCollider, PointerHit, PointerHitStatus},
        viewport::ViewportUtils,
    },
    widgets::{
        WidgetButton, WidgetClick,
        button::Button,
        check_button::{CheckButton, CheckButtonDescriptor},
        color_picker::ColorPicker,
        menu::{MenuDescriptor, MenuEntryDescriptor},
    },
    world::{Descriptor, Element, Handle, World, WorldError},
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
                world.remove(stroke).unwrap();
            }
        });

        let collider = world.insert(PointerCollider::fullscreen(-100));

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world| {
            if let PointerKind::Touch(_) = event.pointer {
                let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();
                match event.status {
                    PointerHitStatus::Press => {
                        viewport_utils.anchor_on_screen(world, event.screen);
                        viewport_utils.locked(true);
                    }
                    PointerHitStatus::Moving => {}
                    PointerHitStatus::Release => {
                        viewport_utils.locked(false);
                    }
                }
                return;
            }

            if let PointerHitStatus::Moving | PointerHitStatus::Press = event.status {
                let mut stroke = world.fetch_mut(this).unwrap();

                let tool = world.single_fetch::<ModifiersTool>().unwrap();
                if tool.modifiers.state().alt_key() {
                    stroke.pick(event.position, world);
                } else {
                    stroke.draw(event.position, world);
                }
            }
        });

        // test //

        world.insert(ColorPicker {
            rect: Rectangle::new(0, 0, 30, 30),
            color: Default::default(),
        });

        let button = world.build(CheckButtonDescriptor {
            rect: Rectangle::new(-60, 0, -30, 30),
            checked: false,
            order: 10,
        });

        world.observer(button, move |WidgetClick, world| {
            let mut button = world.fetch_mut(button).unwrap();
            button.checked = !button.checked;
        });

        let button = world.insert(Button {
            rect: Rectangle::new(-120, 0, -90, 30),
            order: 10,
        });

        world.observer(button, move |WidgetClick, world| {
            world.trigger(collider, &PointerMenu(Position::new(-90, 30)));
        });

        world.observer(collider, move |&PointerMenu(position), world| {
            let menu = world.build(MenuDescriptor {
                position,
                entry_width: 400,
                entry_height: 40,
                entry_pad: 5,
            });

            let collider = world.insert(PointerCollider::fullscreen(80));

            world.dependency(collider, menu);

            world.observer(collider, move |event: &PointerHit, world| {
                if let PointerHitStatus::Press | PointerHitStatus::Moving = event.status {
                    return;
                };

                let menu = world.fetch(menu).unwrap();

                if !event.position.within(menu.menu_rect()) {
                    let menu = menu.handle();
                    world.queue(move |world| {
                        world.remove(menu).unwrap();
                    });
                }
            });

            world.observer(collider, move |&PointerMenu(_), world| {
                world.queue(move |world| {
                    world.remove(menu).unwrap();
                });
            });

            type Entries<const N: usize> = [(&'static str, for<'w> fn(&'w World, Position)); N];
            let entries: Entries<_> = [
                ("Label", |world: &World, position| {
                    world.build(TextDescriptor {
                        rect: Rectangle {
                            origin: position,
                            extend: Size::new(300, 40),
                        },
                        text: "LnDrawer",
                        ..Default::default()
                    });
                }),
                ("Palette", |world, position| {
                    world.build(PaletteDescriptor {
                        position,
                        ..Default::default()
                    });
                }),
                ("Logo", |world, position| {
                    let rect = Rectangle {
                        origin: position,
                        extend: Size::splat(100),
                    };

                    let bytes = include_bytes!("../res/icon_hicolor.png");

                    world.build(CanvasDescriptor::from_bytes(rect, 0, bytes).unwrap());
                }),
                ("Check Button", |world, position| {
                    let button = world.build(CheckButtonDescriptor {
                        rect: Rectangle {
                            origin: position,
                            extend: Size::splat(100),
                        },
                        checked: false,
                        order: 10,
                    });

                    world.observer(button, move |WidgetClick, world| {
                        let mut button = world.fetch_mut(button).unwrap();
                        button.checked = !button.checked;
                    });
                }),
                ("Simple Noise", |world, position| {
                    world.build(SimpleNoiseDescriptor { position });
                }),
                ("", |_, _| {}),
                ("  World save", |world, _position| {
                    crate::save::save_into_file(world);
                }),
                ("  World read", |world, _position| {
                    crate::save::load_from_file(world);
                }),
                ("", |_, _| {}),
                ("Switch transparency", |world, _| {
                    let mut render = world.single_fetch_mut::<Render>().unwrap();
                    if render.clear_color == Color::TRANSPARENT {
                        render.clear_color = Color::BLACK;
                    } else if render.clear_color == Color::BLACK {
                        render.clear_color = Color::TRANSPARENT;
                    }
                }),
                ("Switch title bar", |world, _| {
                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    let decorated = lnwindow.window.is_decorated();
                    lnwindow.window.set_decorations(!decorated);
                }),
                ("Color Picker", |world, position| {
                    world.insert(ColorPicker {
                        rect: Rectangle {
                            origin: position,
                            extend: Size::splat(50),
                        },
                        color: Default::default(),
                    });
                }),
                ("Hook!", |world, position| {
                    let button = world.insert(Button {
                        rect: Rectangle {
                            origin: position,
                            extend: Size::splat(50),
                        },
                        order: 100,
                    });

                    let mut anim_stock = None;
                    world.observer(button, move |event, world| match event {
                        WidgetButton::ButtonPress => {
                            let button = world.fetch(button).unwrap();
                            let current = button.rect.origin;

                            let mut viewport_utils =
                                world.single_fetch_mut::<ViewportUtils>().unwrap();
                            viewport_utils.anchor(world, button.rect.origin.into_fract());
                            viewport_utils.locked(true);

                            let anim = world.build(OnceAnimationDescriptor {
                                animation: AnimationDescriptor {
                                    src: [current.x as f32, current.y as f32],
                                    dst: if current.x.abs() < 50 && current.y.abs() < 50 {
                                        if position.x.abs() < 500 && position.y.abs() < 500 {
                                            [position.x as f32 + 1500.0, position.y as f32]
                                        } else {
                                            [position.x as f32, position.y as f32]
                                        }
                                    } else {
                                        [0.0, 0.0]
                                    },
                                    factor: 5.0,
                                },
                                widget: button.handle(),
                                action: |mut button, world, val| {
                                    button.rect.origin =
                                        Position::new(val[0].round() as i32, val[1].round() as i32);

                                    let mut viewport_utils =
                                        world.single_fetch_mut::<ViewportUtils>().unwrap();
                                    viewport_utils.anchor(world, button.rect.origin.into_fract());
                                },
                            });

                            if let Some(old) = anim_stock.replace(anim) {
                                let _ = world.remove(old);
                            }
                        }
                        WidgetButton::ButtonRelease => {
                            let mut viewport_utils =
                                world.single_fetch_mut::<ViewportUtils>().unwrap();
                            viewport_utils.locked(false);

                            if let Some(old) = anim_stock.take() {
                                let _ = world.remove(old);
                            }
                        }
                    });
                }),
            ];

            for (i, (desc, action)) in entries.into_iter().enumerate() {
                let entry = world.build(MenuEntryDescriptor { menu });

                world.queue(move |world| {
                    let menu = world.fetch(menu).unwrap();
                    let rect = menu.entry_rect(i as f32).expand(-5);
                    let rect = rect.with_left(rect.left() + 30);

                    let text = world.build(TextDescriptor {
                        text: desc,
                        rect,
                        order: 120,
                        metrics: Metrics::new(20.0, menu.entry_height as f32 - 10.0),
                        ..Default::default()
                    });

                    world.dependency(text, menu.handle());
                });

                world.observer(entry, move |WidgetClick, world| {
                    world.queue(move |world| {
                        let menu = world.fetch(menu).unwrap();
                        action(world, menu.position);
                        let menu = menu.handle();
                        world.queue(move |world| {
                            world.remove(menu).unwrap();
                        });
                    });
                });
            }
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

        if let Err(WorldError::SingletonNoSuch(_)) = world.single::<RoundBrush>() {
            world.insert(RoundBrush::new(&world.single_fetch().unwrap()));
        }

        let canvas = chunk.canvas;
        let color = self.color;
        world.queue(move |world| {
            let canvas = world.fetch(canvas).unwrap();
            let brush = world.single_fetch::<RoundBrush>().unwrap();
            let render = world.single_fetch::<Render>().unwrap();

            let (wx, wy) = StrokeLayer::world_to_texture(point, canvas.rect);
            brush.draw(&canvas.texture, [wx as f32, wy as f32], 6.0, 0.5, &render);

            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.request_redraw();
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

            world.foreach_fetch_mut::<Palette>(|mut palette| {
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

        world.observer(button, |WidgetClick, world| {});

        world.insert(StrokeToolbox { button })
    }
}

impl Element for StrokeToolbox {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(self.button, this);
    }
}
