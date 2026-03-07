mod canvas;
mod round_brush;

use cosmic_text::Metrics;
use hashbrown::HashMap;
use palette::{Srgba, WithAlpha};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, Color, Queue, ShaderStages,
};
use winit::event::PointerKind;

use crate::{
    animation::{AnimationDescriptor, OnceAnimationDescriptor},
    elements::{noise::SimpleNoiseDescriptor, palette::PaletteDescriptor},
    lnwin::Lnwindow,
    measures::{Position, Rectangle, Size},
    render::{Render, canvas::CanvasDescriptor, text::TextDescriptor},
    stroke::{
        canvas::{CanvasChunk, CanvasChunkPipeline},
        round_brush::{RoundBrush, RoundBrushPipeline, RoundBrushUniform},
    },
    tools::{
        mouse::PointerMenu,
        pointer::{PointerCollider, PointerHit, PointerHitStatus},
        viewport::ViewportUtils,
    },
    widgets::{
        WidgetButton, WidgetClick,
        button::Button,
        check_button::CheckButtonDescriptor,
        color_picker::ColorPicker,
        menu::{MenuDescriptor, MenuEntryDescriptor},
    },
    world::{Element, Handle, World},
};

const CHUNK_SIZE: u32 = 512;

pub struct StrokeLayer {
    pub chunks: HashMap<(i32, i32), Handle<CanvasChunk>>,
    pub front_color: Srgba,
    pub brush: RoundBrush,
    pub modifier: BrushModifier,

    draw: BindGroup,
    draw_data: Buffer,

    queue: Queue,
}

pub struct BrushModifier {
    pub min_size: f32,
    pub max_size: f32,
    pub size_force_exp: f32,
    pub softness: f32,
}

struct Draw {
    position: Position,
    force: f32,
    color: Srgba,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    color: [f32; 4],
    position: [i32; 2],
    force: f32,
    _pad: u32,
}

impl Element for StrokeLayer {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.foreach::<StrokeLayer>(|stroke| {
            // need to keep it singleton
            if stroke != this {
                world.remove(stroke).unwrap();
            }
        });

        self.attach_pointer(world, this);
    }
}

impl StrokeLayer {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let draw = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("draw"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let canvas_chunk_pipeline = CanvasChunkPipeline::new(world);
        let round_brush_pipeline =
            RoundBrushPipeline::new(&render, &canvas_chunk_pipeline.compute, &draw);

        let draw_data = device.create_buffer(&BufferDescriptor {
            label: Some("draw_data"),
            size: size_of::<DrawUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draw = device.create_bind_group(&BindGroupDescriptor {
            label: Some("draw"),
            layout: &draw,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &draw_data,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let default_brush = RoundBrush::new(&render, &round_brush_pipeline);
        let default_modifier = BrushModifier {
            min_size: 2.0,
            max_size: 6.0,
            size_force_exp: 2.0,
            softness: 0.5,
        };

        world.insert(canvas_chunk_pipeline);
        world.insert(round_brush_pipeline);

        StrokeLayer {
            chunks: HashMap::new(),
            front_color: palette::named::BLACK.with_alpha(1.0).into_format(),
            brush: default_brush,
            modifier: default_modifier,
            draw,
            draw_data,
            queue: render.queue.clone(),
        }
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
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
                let mut this = world.fetch_mut(this).unwrap();

                let draw = Draw {
                    position: event.position,
                    force: event.data.force.unwrap_or(1.0),
                    color: this.front_color,
                };

                this.draw(draw, world);
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

    fn draw(&mut self, draw: Draw, world: &World) {
        let chunk = (
            draw.position.x.div_euclid(CHUNK_SIZE as i32),
            draw.position.y.div_euclid(CHUNK_SIZE as i32),
        );

        match self.chunks.get(&chunk) {
            Some(&canvas) => {
                let canvas = world.fetch(canvas).unwrap();

                self.draw_chunk(&canvas, draw, world);
            }
            None => {
                let canvas = CanvasChunk::new(world, chunk);

                self.draw_chunk(&canvas, draw, world);

                let canvas = world.insert(canvas);
                self.chunks.insert(chunk, canvas);
            }
        }
    }

    fn draw_chunk(&mut self, canvas: &CanvasChunk, draw: Draw, world: &World) {
        let brush_pipeline = world.single_fetch::<RoundBrushPipeline>().unwrap();
        let render = world.single_fetch::<Render>().unwrap();

        self.queue.write_buffer(
            &self.draw_data,
            0,
            bytemuck::bytes_of(&DrawUniform {
                color: [
                    draw.color.red,
                    draw.color.green,
                    draw.color.blue,
                    draw.color.alpha,
                ],
                position: draw.position.into_array(),
                force: draw.force,
                _pad: 0,
            }),
        );

        let modifier = &self.modifier;
        self.queue.write_buffer(
            &self.brush.brush_data,
            0,
            bytemuck::bytes_of(&RoundBrushUniform {
                size: modifier.min_size
                    + (modifier.max_size - modifier.min_size)
                        * draw.force.powf(modifier.size_force_exp),
                softness: modifier.softness,
            }),
        );

        self.brush
            .draw(&render, &brush_pipeline, &canvas.compute, &self.draw);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}
