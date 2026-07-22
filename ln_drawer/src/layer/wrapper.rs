use std::{
    sync::mpsc::channel,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use glam::{DVec2, IVec2, UVec2, Vec2};
use ln_world::{Element, Handle, World};
use palette::Srgba;
use winit::event::PointerKind;

use crate::{
    layer::{
        Layer, LayerConfig,
        brush::{Brush, BrushConfig},
        stream::{self, StreamConfig, ThreadInput, ThreadOutput},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged, CameraUtils, UICamera},
        rounded::{RoundedRect, RoundedRectDescriptor},
    },
    save::{Autosave, SaveDatabase},
    stroke::{interpolate::Draw, modifier::Modifier},
    tools::{
        collider::ToolCollider,
        pointer::{PointerHover, PointerHoverStatus},
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    widgets::{WidgetEnabled, WidgetRectangle},
};

pub struct LayerDebugMessage(pub String);

pub struct LayerWrapper {
    pub layer: Layer,
    pub brush: Brush,
    pub render_debugging: bool,
    pub erase: bool,

    brush_preview: Handle<RoundedRect>,

    thread_tx: std::sync::mpsc::Sender<ThreadInput>,
    thread_rx: std::sync::mpsc::Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,
}

impl LayerWrapper {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        let layer = Layer::new(LayerConfig {
            device: render.device.clone(),
            queue: render.queue.clone(),
            surface_format: render.config.format,
            mipmap_levels: 8,
            chunk_size: 512,
            controlled: true,
            camera_bind_layout: camera_bind.layout.clone(),
        });

        let brush = Brush::new(BrushConfig {
            device: render.device.clone(),
            queue: render.queue.clone(),
            chunk_draw_layout: layer.chunk_draw_layout.clone(),
            scratch: LayerConfig {
                device: render.device.clone(),
                queue: render.queue.clone(),
                surface_format: render.config.format,
                mipmap_levels: 1,
                chunk_size: 512,
                controlled: false,
                camera_bind_layout: camera_bind.layout.clone(),
            },
        });

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();

        let (input_tx, input_rx) = channel();
        let (output_tx, output_rx) = channel();

        let stream_config = StreamConfig {
            database,
            device: render.device.clone(),
            queue: render.queue.clone(),
            chunk_render_layout: layer.chunk_render_layout.clone(),
            chunk_draw_layout: layer.chunk_draw_layout.clone(),
            chunk_size: layer.chunk_size,
            mipmap_levels: layer.mipmap_levels,
        };

        let camera = world.single_fetch::<Camera>().unwrap();
        input_tx
            .send(ThreadInput::SetStreamCamera(
                camera.zoom,
                camera.size,
                camera.center,
            ))
            .unwrap();

        let thread = std::thread::spawn(move || {
            stream::loading_thread(stream_config, input_rx, output_tx).unwrap();
        });

        let ui_camera = world.single_fetch::<UICamera>().unwrap();
        let brush_preview = world.enter(ui_camera.0, || {
            world.build(RoundedRectDescriptor {
                rect: Rectangle::new_half(IVec2::new(0, 0), UVec2::new(1, 1)),
                color: Srgba::new(0.5, 0.5, 0.5, 0.4),
                shrink: 0.5,
                value: 0.5,
                shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.3),
                shadow_offset: Vec2::ZERO,
                shadow_blur: 30.0,
                visible: false,
                vertex_extend: 80,
                order: -10,
                ..Default::default()
            })
        });

        LayerWrapper {
            layer,
            brush,
            render_debugging: false,
            erase: false,
            brush_preview,
            thread_tx: input_tx,
            thread_rx: output_rx,
            thread: Some(thread),
        }
    }

    fn attach_touch(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider::fullscreen(-100));
        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHover, world| {
            if let PointerKind::Touch(_) = event.pointer.kind {
                return;
            }

            let this = world.fetch(this).unwrap();
            let ui_camera = world.single_fetch::<UICamera>().unwrap();
            world.enter(ui_camera.0, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                let mut brush_preview = world.fetch_mut(this.brush_preview).unwrap();
                brush_preview.desc.shadow_offset = event.pointer.tilt * 48.0;
                world.queue_trigger(
                    this.brush_preview,
                    WidgetRectangle(Rectangle::new_half(
                        camera
                            .screen_to_world_absolute(event.pointer.screen)
                            .q32_round(),
                        UVec2::new(1, 1),
                    )),
                );

                match event.status {
                    PointerHoverStatus::Enter => {
                        world.queue_trigger(this.brush_preview, WidgetEnabled(true));
                    }
                    PointerHoverStatus::Moving => {}
                    PointerHoverStatus::Leave => {
                        world.queue_trigger(this.brush_preview, WidgetEnabled(false));
                    }
                }
            });
        });

        let mut pinch_distance = None;
        let mut drag_start = None;
        let mut temp_erase_mode = None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let primary = event.members.first().unwrap();

            if matches!(event.active.pointer, PointerKind::Touch(_)) || event.members.len() != 1 {
                let mut sum = [0f64; 2];
                for member in &event.members {
                    sum[0] += member.screen[0];
                    sum[1] += member.screen[1];
                }

                let cnt = event.members.len() as f64;
                let center = [sum[0] / cnt, sum[1] / cnt];

                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                match event.active.status {
                    MultiTouchStatus::Press => {
                        camera_utils.locked(false);
                        camera_utils.cursor(world, center);
                        camera_utils.anchor_on_screen(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Holding => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Release => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(false);
                    }
                }

                if event.members.len() == 2 {
                    let first = event.members.first().unwrap().screen;
                    let last = event.members.last().unwrap().screen;

                    let (x, y) = (first[0] - last[0], first[1] - last[1]);
                    let cur = (x * x + y * y).sqrt();
                    let prev = pinch_distance.get_or_insert(cur);
                    camera_utils.zoom_delta(world, i64::q32_from_f64((cur - *prev) * 2.0));
                    *prev = cur;
                } else {
                    pinch_distance = None;
                }
            } else if let MultiTouchStatus::Holding | MultiTouchStatus::Press = primary.status {
                let this = &mut *world.fetch_mut(this).unwrap();

                if let MultiTouchStatus::Press = primary.status
                    && !this.erase
                {
                    drag_start = Some((primary.screen, Instant::now()));
                }

                if let Some((start, timer)) = drag_start {
                    const DRAG_DISTANCE: f64 = 0.01;
                    const ERASE_TIMER: f64 = 0.8;
                    const ERASE_FORCE_THRESHOLD: f32 = 0.6;
                    const TEMP_ERASE_MODIFIER: Modifier = Modifier {
                        min_size: 5.0,
                        max_size: 15.0,
                        size_force_exp: 1.0,
                        min_flow: 0.5,
                        max_flow: 1.0,
                        flow_force_exp: 1.0,
                        softness: 0.5,
                        color: Srgba::new(1.0, 1.0, 1.0, 1.0),
                    };

                    if DVec2::from_array(primary.screen).distance(DVec2::from_array(start))
                        > DRAG_DISTANCE
                    {
                        drag_start = None;
                    } else if timer.elapsed() > Duration::from_secs_f64(ERASE_TIMER) {
                        if primary.data.force.unwrap_or(1.0) >= ERASE_FORCE_THRESHOLD {
                            this.brush
                                .submit_stream(&mut this.layer, &this.thread_tx, this.erase);
                            temp_erase_mode = Some(this.brush.modifier);
                            this.erase = true;
                            this.brush.modifier = TEMP_ERASE_MODIFIER;
                            drag_start = None;
                        } else {
                            drag_start = None;
                        }
                    }
                }

                this.brush.paint(Draw {
                    position: primary.position,
                    force: primary.data.force.unwrap_or(1.0),
                });

                this.brush.request_stream(&mut this.layer, &this.thread_tx);

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                lnwindow.window.request_redraw();
            } else {
                let this = &mut *world.fetch_mut(this).unwrap();

                if let Some(ori) = temp_erase_mode {
                    temp_erase_mode = None;
                    this.erase = false;
                    this.brush.modifier = ori;
                }

                this.brush
                    .submit_stream(&mut this.layer, &this.thread_tx, this.erase);
            }
        });
    }

    fn process_stream(&mut self, world: &World) {
        while let Ok(output) = self.thread_rx.try_recv() {
            match output {
                ThreadOutput::ThreadDebugMessage(msg) => {
                    world.queue_trigger(
                        world.single::<LayerWrapper>().unwrap(),
                        LayerDebugMessage(msg),
                    );
                }
                ThreadOutput::Insert(key, chunk_bind) => {
                    self.layer.chunks.insert(key, chunk_bind);
                }
                ThreadOutput::Remove(key) => {
                    self.layer.chunks.remove(&key);
                }
            }
        }
    }
}

impl Drop for LayerWrapper {
    fn drop(&mut self) {
        self.thread_tx.send(ThreadInput::Abort).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Element for LayerWrapper {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.single::<LayerWrapper>().unwrap();

        let save = world.insert(Autosave(Box::new(move |world, _write| {
            let this = world.single_fetch::<LayerWrapper>().unwrap();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let camera = world.single::<Camera>().unwrap();
        world.observer(camera, move |_: &CameraPositionChanged, world| {
            let this = world.single_fetch::<LayerWrapper>().unwrap();
            let camera = world.single_fetch::<Camera>().unwrap();

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.size,
                    camera.center,
                ))
                .unwrap();
        });

        self.attach_touch(world, this);

        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this).unwrap();
                this.process_stream(world);

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(move |world, rpass, diagnosis| {
                let this = world.single_fetch::<LayerWrapper>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                rpass.write_timestamp(&diagnosis.query, 2);

                this.layer
                    .render(rpass, &camera, this.render_debugging, false);

                this.brush
                    .scratch
                    .render(rpass, &camera, this.render_debugging, this.erase);

                rpass.write_timestamp(&diagnosis.query, 3);

                diagnosis.slots.push(((2, 3), "layer_wrapper"));
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
