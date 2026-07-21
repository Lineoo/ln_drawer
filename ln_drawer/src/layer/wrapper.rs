use std::{sync::mpsc::channel, thread::JoinHandle, time::{Duration, Instant}};

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
    stroke::{
        dirty::Dirty,
        interpolate::{Draw, Interpolation},
        modifier::{DrawProcessedStorage, Modifier},
    },
    tools::{
        collider::ToolCollider,
        pointer::{PointerHover, PointerHoverStatus},
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    widgets::{WidgetEnabled, WidgetRectangle},
};

const DEFAULT_INTERPOLATION: Interpolation = Interpolation {
    step: |draw| draw.size / 5.0,
};
const DEFAULT_MODIFIER: Modifier = Modifier {
    min_size: 0.5,
    max_size: 6.0,
    size_force_exp: 1.0,
    min_flow: 0.1,
    max_flow: 1.0,
    flow_force_exp: 2.0,
    softness: 0.2,
    color: Srgba::new(0.0, 0.0, 0.0, 1.0),
};
const DEFAULT_DIRTY: Dirty = Dirty {
    bounding: |draw| {
        Rectangle::new_half(
            draw.position.q32_round(),
            UVec2::splat((draw.size * 2.0).ceil() as u32),
        )
    },
};

pub struct LayerDebugMessage(pub String);

pub struct LayerWrapper {
    pub layer: Layer,
    pub brush: Brush,
    pub render_debugging: bool,

    pub erase: bool,
    pub interpolation: Interpolation,
    pub modifier: Modifier,
    pub dirty: Dirty,

    prev: Option<Draw>,
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
            interpolation: DEFAULT_INTERPOLATION,
            modifier: DEFAULT_MODIFIER,
            dirty: DEFAULT_DIRTY,
            prev: None,
            brush_preview,
            thread_tx: input_tx,
            thread_rx: output_rx,
            thread: Some(thread),
        }
    }

    fn paint_gpu(
        &mut self,
        dirty: Rectangle,
        draws: &[DrawProcessedStorage],
        erase: bool,
    ) {
        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

        if !self.layer.validate_chunks(dirty) {
            if self.layer.controlled {
                for key in missing_chunks(
                    dirty,
                    self.layer.chunk_size,
                    self.layer.mipmap_levels,
                    &self.layer.chunks,
                ) {
                    self.thread_tx.send(ThreadInput::RequestReal(key)).unwrap();
                }
            } else {
                self.layer.prepare_chunks(dirty);
            }
            return;
        }

        let (src, dst) =
            super::rect_to_chunks(dirty, 0, self.layer.chunk_size);
        let mut paint_chunks = Vec::new();
        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                if let Some(chunk) = self.layer.chunks.get(&(x, y, 0)) {
                    paint_chunks.push(((x, y, 0), &chunk.draw));
                }
            }
        }

        self.brush.paint(dirty, draws, &paint_chunks, erase);
        self.layer.generate_mipmaps(dirty);

        for mipmap in 0..self.layer.mipmap_levels {
            let (s, d) = super::rect_to_chunks(dirty, mipmap, self.layer.chunk_size);
            for x in s.0..d.0 {
                for y in s.1..d.1 {
                    self.thread_tx
                        .send(ThreadInput::MarkUnsaved((x, y, mipmap)))
                        .unwrap();
                }
            }
        }
    }

    fn paint(&mut self, next: Draw, world: &World) {
        let mut draw_buf = Vec::new();
        let curr = self
            .interpolation
            .interpolate(self.prev, next, &self.modifier, &mut draw_buf);
        self.prev = Some(curr);

        let dirty = self.dirty.compute(curr.position.q32_round(), &draw_buf);

        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        self.paint_gpu(dirty, &draw_stg, self.erase);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
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
                let mut this = world.fetch_mut(this).unwrap();

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
                            temp_erase_mode = Some(this.modifier);
                            this.erase = true;
                            this.modifier = TEMP_ERASE_MODIFIER;
                            this.prev = None;
                            drag_start = None;
                        } else {
                            drag_start = None;
                        }
                    }
                }

                let target = Draw {
                    position: primary.position,
                    force: primary.data.force.unwrap_or(1.0),
                };

                this.paint(target, world);
            } else {
                let mut this = world.fetch_mut(this).unwrap();

                if let Some(ori) = temp_erase_mode {
                    temp_erase_mode = None;
                    this.erase = false;
                    this.modifier = ori;
                }

                this.prev = None;
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

fn missing_chunks(
    dirty: Rectangle,
    chunk_size: u32,
    mipmap_levels: u8,
    chunks: &hashbrown::HashMap<super::ChunkKey, super::Chunk>,
) -> Vec<super::ChunkKey> {
    let mut missing = Vec::new();
    for mipmap in 0..mipmap_levels {
        let (src, dst) = super::rect_to_chunks(dirty, mipmap, chunk_size);
        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                let key = (x, y, mipmap);
                if !chunks.contains_key(&key) {
                    missing.push(key);
                }
            }
        }
    }
    missing
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
            draw: Some(Box::new(move |world, rpass| {
                let this = world.single_fetch::<LayerWrapper>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                this.layer.render(rpass, &camera, this.render_debugging);
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
