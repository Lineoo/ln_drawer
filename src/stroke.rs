mod canvas;
mod round_brush;

use cosmic_text::Metrics;
use hashbrown::HashMap;
use palette::{Srgba, WithAlpha};
use serde_bytes::ByteBuf;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, Device,
    Extent3d, MapMode, Origin3d, Queue, ShaderStages, TexelCopyBufferInfoBase,
    TexelCopyBufferLayout, TexelCopyTextureInfoBase, TextureAspect, wgt::PollType,
};
use winit::event::PointerKind;

use crate::{
    animation::{AnimationDescriptor, OnceAnimationDescriptor},
    elements::{noise::SimpleNoiseDescriptor, palette::PaletteDescriptor},
    lnwin::Lnwindow,
    measures::{Fract, Position, PositionFract, Rectangle, Size},
    render::{Render, canvas::CanvasDescriptor, text::TextDescriptor},
    save::{AutosaveRequest, SaveControl, SaveExpand, SaveScheduler},
    stroke::{
        canvas::{CanvasChunk, CanvasChunkPipeline},
        round_brush::{RoundBrush, RoundBrushPipeline, RoundBrushStorage},
    },
    tools::{
        collider::PointerCollider,
        mouse::PointerMenu,
        pointer::{PointerHit, PointerHitStatus},
        viewport::ViewportUtils,
    },
    widgets::{
        WidgetButton, WidgetClick,
        button::Button,
        check_button::CheckButtonDescriptor,
        color_picker::ColorPicker,
        menu::{MenuDescriptor, MenuEntryDescriptor},
    },
    world::{Element, Handle, World, WorldError},
};

const CHUNK_SIZE: u32 = 512;
const MAX_STROKE: u64 = 1000;

pub struct StrokeLayer {
    pub chunks: HashMap<(i32, i32), Handle<CanvasChunk>>,
    pub front_color: Srgba,
    pub brush: RoundBrush,
    pub modifier: BrushModifier,

    archive: Archive,
    current: Option<BrushInstance>,

    draw: BindGroup,
    draw_data: Buffer,

    device: Device,
    queue: Queue,
}

pub struct BrushModifier {
    pub min_size: f32,
    pub max_size: f32,
    pub size_force_exp: f32,
    pub min_flow: f32,
    pub max_flow: f32,
    pub flow_force_exp: f32,
    pub softness: f32,
    pub step: Option<f32>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Archive {
    chunks: HashMap<(i32, i32), ByteBuf>,
}

#[derive(Clone, Copy)]
struct BrushInstance {
    position: PositionFract,
    force: f32,
    step: Fract,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    dirty_coords: [i32; 2],
    stroke_count: u32,
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
    pub fn init(world: &mut World) {
        world.insert(SaveExpand {
            name: "stroke".into(),
            expand: Box::new(|world, control| {
                let mut stroke = StrokeLayer::new(world);

                world.queue(move |world| {
                    let control = world.fetch(control).unwrap();
                    let bytes = control.read(world);

                    stroke.archive = postcard::from_bytes::<Archive>(&bytes).unwrap();
                    for (&chunk, bytes) in &stroke.archive.chunks {
                        let canvas = CanvasChunk::new(world, chunk);
                        stroke.queue.write_texture(
                            TexelCopyTextureInfoBase {
                                texture: &canvas.texture,
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            &bytes,
                            TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(CHUNK_SIZE * 4),
                                rows_per_image: Some(CHUNK_SIZE),
                            },
                            Extent3d {
                                width: CHUNK_SIZE,
                                height: CHUNK_SIZE,
                                depth_or_array_layers: 1,
                            },
                        );

                        let canvas = world.insert(canvas);
                        stroke.chunks.insert(chunk, canvas);
                    }

                    let control = control.handle();
                    let stroke = world.insert(stroke);
                    StrokeLayer::register_saving(world, stroke, control);
                });
            }),
        });

        world.flush();

        if let Err(WorldError::SingletonNoSuch(_)) = world.single::<StrokeLayer>() {
            let stroke = world.insert(StrokeLayer::new(world));
            let control = SaveControl::create("stroke".into(), world, &[]);
            StrokeLayer::register_saving(world, stroke, control);
        }
    }

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
            min_size: 0.5,
            max_size: 6.0,
            size_force_exp: 1.0,
            min_flow: 0.1,
            max_flow: 1.0,
            flow_force_exp: 2.0,
            softness: 0.5,
            step: None,
        };

        world.insert(canvas_chunk_pipeline);
        world.insert(round_brush_pipeline);

        StrokeLayer {
            chunks: HashMap::new(),
            front_color: palette::named::BLACK.with_alpha(1.0).into_format(),
            brush: default_brush,
            modifier: default_modifier,
            archive: Archive {
                chunks: HashMap::new(),
            },
            current: None,
            draw,
            draw_data,
            device: render.device.clone(),
            queue: render.queue.clone(),
        }
    }

    fn register_saving(world: &World, this: Handle<Self>, control: Handle<SaveControl>) {
        let scheduler = world.single::<SaveScheduler>().unwrap();
        world.observer(scheduler, move |AutosaveRequest, world| {
            let mut this = world.fetch_mut(this).unwrap();

            this.device_readback(world);
            let bytes = postcard::to_stdvec(&this.archive).unwrap();

            let control = world.fetch(control).unwrap();
            control.write(world, &bytes);
        });
    }

    fn device_readback(&mut self, world: &World) {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut cnt = 0;
        for (&chunk, &canvas) in &self.chunks {
            let mut canvas = world.fetch_mut(canvas).unwrap();

            if !canvas.changed {
                continue;
            }

            cnt += 1;
            canvas.changed = false;

            let readback_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("canvas_readback"),
                size: (CHUNK_SIZE * CHUNK_SIZE * 4) as u64,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("canvas_readback"),
                });

            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfoBase {
                    texture: &canvas.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyBufferInfoBase {
                    buffer: &readback_buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(CHUNK_SIZE * 4),
                        rows_per_image: Some(CHUNK_SIZE),
                    },
                },
                Extent3d {
                    width: CHUNK_SIZE,
                    height: CHUNK_SIZE,
                    depth_or_array_layers: 1,
                },
            );

            let command = encoder.finish();

            self.queue.submit([command]);

            let inner = readback_buffer.clone();
            let tx = tx.clone();
            readback_buffer.map_async(MapMode::Read, .., move |ret| {
                ret.unwrap();

                let view = inner.get_mapped_range(..);
                tx.send((chunk, view.to_vec())).unwrap();
            });
        }

        self.device.poll(PollType::wait_indefinitely()).unwrap();

        while cnt != 0
            && let Ok((chunk, bytes)) = rx.recv()
        {
            self.archive.chunks.insert(chunk, bytes.into());
            cnt -= 1;
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
            } else if let PointerHitStatus::Moving | PointerHitStatus::Press = event.status {
                let mut this = world.fetch_mut(this).unwrap();
                let target = BrushInstance {
                    position: event.position.into_fract(),
                    force: event.data.force.unwrap_or(1.0),
                    step: Fract::ZERO,
                };

                let handle = this.handle();
                this.draw(target, world, handle);
            } else {
                world.queue(move |world| {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.current = None;
                });
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
                    if render.clear_color.a == 0.0 {
                        render.clear_color.a = 1.0;
                    } else if render.clear_color.a == 1.0 {
                        render.clear_color.a = 0.0;
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

    fn draw(&mut self, target: BrushInstance, world: &World, this: Handle<Self>) {
        // generate draws //

        let previous = *self.current.get_or_insert(target);
        let mut working = previous;
        let mut brushes = Vec::new();
        let mut dirty_box = Rectangle::new_half(working.position.round(), Size::splat(0));

        while working.position.distance(target.position) >= working.step
            && brushes.len() < MAX_STROKE as usize
        {
            working.position = working.position.move_towards(target.position, working.step);

            let previous_distance = previous.position.distance(target.position).into_f32();
            let working_distance = working.position.distance(target.position).into_f32();
            let progress = match working_distance < 1e-6 {
                true => 1.0,
                false => 1.0 - working_distance / previous_distance,
            };

            working.force = (1.0 - progress) * previous.force + progress * target.force;

            // apply brush modifier //

            let modifier = &self.modifier;

            let size = modifier.min_size
                + (modifier.max_size - modifier.min_size)
                    * working.force.powf(modifier.size_force_exp);

            let flow = modifier.min_flow
                + (modifier.max_flow - modifier.min_flow)
                    * working.force.powf(modifier.flow_force_exp);

            let softness = modifier.softness;

            let color = self.front_color;

            brushes.push(RoundBrushStorage {
                color: [color.red, color.green, color.blue, color.alpha],
                position: working.position.round().into_array(),
                force: working.force,
                size,
                softness,
                flow,
                _pad: 0,
            });

            dirty_box = dirty_box.grow(Rectangle::new_half(
                working.position.round(),
                Size::splat((size * 2.0).ceil() as u32),
            ));

            working.step = Fract::from_f32(match self.modifier.step {
                Some(step) => step,
                None => size / 5.0,
            });
        }

        self.current = Some(working);

        let draw = DrawUniform {
            dirty_coords: dirty_box.origin.into_array(),
            stroke_count: brushes.len() as u32,
            _pad: 0,
        };

        self.draw_batch(dirty_box, draw, brushes, world, this);
    }

    fn draw_batch(
        &mut self,
        dirty_box: Rectangle,
        draw: DrawUniform,
        brushes: Vec<RoundBrushStorage>,
        world: &World,
        this: Handle<Self>,
    ) {
        let chunk_src = (
            dirty_box.left().div_euclid(CHUNK_SIZE as i32),
            dirty_box.down().div_euclid(CHUNK_SIZE as i32),
        );

        let chunk_dst = (
            (dirty_box.right() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
            (dirty_box.up() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
        );

        let mut chunks = Vec::new();
        for chunk_x in chunk_src.0..chunk_dst.0 {
            for chunk_y in chunk_src.1..chunk_dst.1 {
                chunks.push(match self.chunks.get(&(chunk_x, chunk_y)) {
                    Some(&canvas) => canvas,
                    None => {
                        let canvas = CanvasChunk::new(world, (chunk_x, chunk_y));

                        let canvas = world.insert(canvas);
                        self.chunks.insert((chunk_x, chunk_y), canvas);

                        canvas
                    }
                });
            }
        }

        world.queue(move |world| {
            for chunk in chunks {
                let mut this = world.fetch_mut(this).unwrap();
                let mut chunk = world.fetch_mut(chunk).unwrap();
                this.draw_chunk(dirty_box, &mut chunk, &draw, &brushes, world);
            }
        });
    }

    fn draw_chunk(
        &mut self,
        dirty_box: Rectangle,
        canvas: &mut CanvasChunk,
        draw: &DrawUniform,
        brushes: &[RoundBrushStorage],
        world: &World,
    ) {
        if dirty_box.extend.w == 0 || dirty_box.extend.h == 0 {
            return;
        }

        let brush_pipeline = world.single_fetch::<RoundBrushPipeline>().unwrap();
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        canvas.changed = true;

        self.queue
            .write_buffer(&self.draw_data, 0, bytemuck::bytes_of(draw));

        self.queue.write_buffer(
            &self.brush.brush_data_array,
            0,
            bytemuck::cast_slice(brushes),
        );

        // start compute pass

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("brush"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("brush"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&brush_pipeline.pipeline);
        cpass.set_bind_group(0, Some(&canvas.compute), &[]);
        cpass.set_bind_group(1, Some(&self.draw), &[]);
        cpass.set_bind_group(2, Some(&self.brush.brush), &[]);

        const WORKGROUP_SIZE: Size = Size::new(16, 16);
        cpass.dispatch_workgroups(
            (dirty_box.extend.w - 1) / WORKGROUP_SIZE.w + 1,
            (dirty_box.extend.h - 1) / WORKGROUP_SIZE.h + 1,
            1,
        );

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}
