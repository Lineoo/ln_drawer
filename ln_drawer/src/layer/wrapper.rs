use std::{
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
};

use glam::{I64Vec2, IVec2, UVec2, Vec4};
use hashbrown::HashMap;
use ln_world::{Element, Handle, World};
use palette::{IntoColor, Srgba};
use wgpu::{CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, RenderPass};

use crate::{
    layer::{
        CacheLayerChunk, CacheLayers, ChunkPool, DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_CAP,
        LayerPipeline, PlainLayer, RawLayer, Standalone,
        brush::{
            BrushParams, Draw, Stroke, blur::BlurBrush, param::BrushParam, pixel::PixelBrush,
            round::RoundBrush, smudge::SmudgeBrush, tint::TintBrush,
        },
        chunks_key_to_rect, create_chunk, create_chunk_texture, rect_chunks_stack_layer_iter,
        stream::{StreamConfig, StreamThread, ThreadInput, ThreadOutput},
        traveler::Traveler,
        zoom_to_mipmap,
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl, RenderExtra, RenderInformation,
        camera::{CameraBind, CameraUpdated, MainCamera, UICamera},
    },
    save::{Autosave, Save},
    widgets::renderer::rrect::RRect,
};

const BRIDGE_CHUNK_SIZE: UVec2 = UVec2::splat(1024);

// TODO rename module to `page`

pub struct LayerDebugMessage(pub String);

pub const BRUSH_PREVIEW_SHADOW_BLUR: i32 = 30;

pub struct BrushConfigurationChanged;

/// A named brush stored in the preset registry.
pub struct BrushPreset {
    /// i18n key of the display name.
    pub label: &'static str,
    pub brush: Box<dyn BrushParams>,
}

/// layer page - the main entry of infinite canvas
///
/// Page optimization comes from three layers:
/// 1. `main` - The raw data layer
/// 2. `merge` - The *view* cache, the same size as the main layer
///     - contains data merged, for example strokes that are still in draw pipeline scratch
/// 3. `mipmap` - The LOD part, contains textures that are mipmapped upwards.
///    - not implemented yet, we are still holding it inside main layer
///
/// This structure also contains:
///
/// - stream IO loader control
/// - brush preview
/// - undo/redo traveler
pub struct LayerPage {
    pub lp: Arc<LayerPipeline>,

    pub cache: CacheLayers,
    raw: RawLayer,
    scratch: PlainLayer,
    bridge: Standalone,
    pool: ChunkPool,

    stroke: Option<Stroke>,

    traveler: Traveler,

    /// Immutable registry of brush presets.
    pub brushes: Vec<BrushPreset>,
    /// Index of the preset the active brush was copied from.
    pub active_brush: usize,

    active: Box<dyn BrushParams>,
    erase: RoundBrush,
    color: Srgba,

    pub debug: bool,

    pub cursor: Handle<RRect>,
    pub cursor_shadow: Handle<RRect>,

    pub thread_tx: Sender<ThreadInput>,
    pub thread_rx: Receiver<ThreadOutput>,
    pub thread: Option<JoinHandle<()>>,
}

impl LayerPage {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>();
        let camera_bind = world.single_fetch::<CameraBind>();

        let lp = Arc::new(LayerPipeline::new(
            render.adapter.clone(),
            render.device.clone(),
            render.queue.clone(),
            render.config.format,
            &camera_bind.layout,
        ));

        let traveler = Traveler::new(lp.clone());

        let database = world.single_fetch::<Save>().db.clone();
        let window = world.single_fetch::<Lnwindow>().window.clone();

        let (input_tx, input_rx) = channel();
        let (output_tx, output_rx) = channel();

        let stream_config = StreamConfig {
            database,
            lp: lp.clone(),
            window,
            page: 0,
            chunk_size: DEFAULT_CHUNK_SIZE,
            mipmap_caps: DEFAULT_MIPMAP_CAP,
        };

        let main_camera = world.single_fetch::<MainCamera>();
        let camera = world.fetch(main_camera.0);
        input_tx
            .send(ThreadInput::SetStreamCamera(
                camera.zoom,
                camera.size,
                camera.center,
            ))
            .unwrap();

        let thread = std::thread::spawn(move || {
            StreamThread::new(stream_config, input_rx, output_tx)
                .run()
                .unwrap();
        });

        let ui_camera = world.single_fetch::<UICamera>().0;

        let cursor = world.insert(RRect {
            rect: Rectangle::new_half(IVec2::new(0, 0), UVec2::new(1, 1)),
            order: -10,
            color: Srgba::new(0.5, 0.5, 0.5, 0.4),
            radius: 0.5,
            width: 0.0,
            enabled: false,
            camera: ui_camera,
        });
        let cursor_shadow = world.insert(RRect {
            rect: Rectangle::new_half(IVec2::new(0, 0), UVec2::new(1, 1)),
            order: -11,
            color: Srgba::new(0.0, 0.0, 0.0, 0.3),
            radius: 0.5,
            width: BRUSH_PREVIEW_SHADOW_BLUR as f32,
            enabled: false,
            camera: ui_camera,
        });

        let color = Srgba::new(0.0, 0.0, 0.0, 1.0);

        let brushes: Vec<BrushPreset> = vec![
            BrushPreset {
                label: "brush.pen",
                brush: Box::new(RoundBrush {
                    size: BrushParam::force_index(0.0, 6.0, 1.0),
                    flow: BrushParam::force_index(0.9, 1.0, 0.5),
                    softness: BrushParam::constant(0.2),
                    spacing: BrushParam::constant(0.1),
                    color,
                    erase: false,
                }),
            },
            BrushPreset {
                label: "brush.soft",
                brush: Box::new(RoundBrush {
                    size: BrushParam::force_index(1.0, 25.0, 1.0),
                    flow: BrushParam::force_index(0.1, 1.0, 1.0),
                    softness: BrushParam::constant(0.5),
                    spacing: BrushParam::constant(0.1),
                    color,
                    erase: false,
                }),
            },
            BrushPreset {
                label: "brush.pixel",
                brush: Box::new(PixelBrush {
                    size: BrushParam::constant(2.0),
                    color,
                    erase: false,
                }),
            },
            BrushPreset {
                label: "brush.blur",
                brush: Box::new(BlurBrush {
                    size: BrushParam::constant(20.0),
                    sigma: BrushParam::constant(3.0),
                    softness: BrushParam::constant(0.3),
                    spacing: BrushParam::constant(0.1),
                }),
            },
            BrushPreset {
                label: "brush.smudge",
                brush: Box::new(SmudgeBrush {
                    size: BrushParam::force_index(1.0, 25.0, 1.0),
                    flow: BrushParam::force_index(0.1, 1.0, 1.0),
                    softness: BrushParam::constant(0.5),
                    spacing: BrushParam::constant(0.1),
                    color,
                    color_ratio: BrushParam::constant(0.4),
                    sample_radius: BrushParam::constant(0.5),
                    sample_rate: BrushParam::constant(0.07),
                }),
            },
            BrushPreset {
                label: "brush.tint",
                brush: Box::new(TintBrush {
                    size: BrushParam::force_index(10.0, 30.0, 1.0),
                    flow: Vec4::new(0.2, 0.7, 0.7, 1.0),
                    softness: BrushParam::constant(0.5),
                    spacing: BrushParam::constant(0.1),
                    color,
                }),
            },
        ];

        let mut active = brushes[0].brush.dup();
        active.set_color(color);

        let bridge_texture = create_chunk_texture(&lp.device, BRIDGE_CHUNK_SIZE);
        let bridge_rect = Rectangle {
            origin: IVec2::ZERO,
            extend: BRIDGE_CHUNK_SIZE,
        };
        let bridge_chunk = create_chunk(&lp.device, &lp.chunk_layout, bridge_texture, bridge_rect);

        LayerPage {
            lp,
            cache: CacheLayers {
                chunks: HashMap::new(),
                chunk_size: DEFAULT_CHUNK_SIZE,
                mipmap_cap: DEFAULT_MIPMAP_CAP,
                thread_tx: input_tx.clone(),
            },
            raw: RawLayer {
                chunks: HashMap::new(),
                chunk_size: DEFAULT_CHUNK_SIZE,
                thread_tx: input_tx.clone(),
            },
            scratch: PlainLayer {
                chunks: HashMap::new(),
                chunk_size: DEFAULT_CHUNK_SIZE,
            },
            bridge: Standalone {
                chunk: bridge_chunk,
                rect: bridge_rect,
            },
            pool: ChunkPool {
                list: Vec::new(),
                chunk_size: DEFAULT_CHUNK_SIZE,
            },
            stroke: None,
            traveler,
            brushes,
            active_brush: 0,
            active,
            erase: RoundBrush {
                size: BrushParam::force_index(5.0, 15.0, 1.0),
                flow: BrushParam::force_index(0.9, 1.0, 0.5),
                softness: BrushParam::constant(0.3),
                spacing: BrushParam::constant(0.1),
                color: Srgba::new(1.0, 1.0, 1.0, 1.0),
                erase: true,
            },
            color,
            debug: false,
            cursor,
            cursor_shadow,
            thread_tx: input_tx,
            thread_rx: output_rx,
            thread: Some(thread),
        }
    }

    pub fn active(&self) -> &dyn BrushParams {
        self.active.as_ref()
    }

    pub fn active_mut(&mut self) -> &mut dyn BrushParams {
        self.active.as_mut()
    }

    pub fn draw_active(&mut self, draw: Draw) {
        let stroke = self
            .stroke
            .get_or_insert_with(|| Stroke::new(self.active.as_ref()));

        let Some(batch) = stroke.batch(self.active.as_ref(), draw) else {
            return;
        };

        stroke.confirm_batch(&batch);
        batch.upload(&self.lp, &mut self.bridge);

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("draw_active"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("draw_active"),
            timestamp_writes: None,
        });

        batch.prepare_scratch(
            &self.lp,
            &mut cpass,
            &self.raw,
            &self.cache,
            &mut self.scratch,
        );

        batch.draw_layers(
            &self.lp,
            &mut cpass,
            &mut self.scratch,
            &mut self.bridge,
            self.active.as_ref(),
        );

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("draw_active_cache"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("draw_active_cache"),
            timestamp_writes: None,
        });

        stroke.upload_dispatch(&self.lp);
        stroke.submit_cache(
            &self.lp,
            &mut cpass,
            &self.scratch,
            &mut self.raw,
            &mut self.cache,
        );

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);
    }

    pub fn draw_erase(&mut self, draw: Draw) {
        let stroke = self.stroke.get_or_insert_with(|| Stroke::new(&self.erase));

        let Some(batch) = stroke.batch(&self.erase, draw) else {
            return;
        };

        stroke.confirm_batch(&batch);
        batch.upload(&self.lp, &mut self.bridge);

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("draw_erase"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("draw_erase"),
            timestamp_writes: None,
        });

        batch.prepare_scratch(
            &self.lp,
            &mut cpass,
            &self.raw,
            &self.cache,
            &mut self.scratch,
        );

        batch.draw_layers(
            &self.lp,
            &mut cpass,
            &mut self.scratch,
            &mut self.bridge,
            &self.erase,
        );

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("draw_erase_cache"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("draw_erase_cache"),
            timestamp_writes: None,
        });

        stroke.upload_dispatch(&self.lp);
        stroke.submit_cache(
            &self.lp,
            &mut cpass,
            &self.scratch,
            &mut self.raw,
            &mut self.cache,
        );

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);
    }

    pub fn submit(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("page_submit"),
        });

        self.traveler.stock(&mut encoder, &self.raw, stroke.dirty);

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("page_submit"),
            timestamp_writes: None,
        });

        stroke.upload_dispatch(&self.lp);
        stroke.submit_raw_cache(
            &self.lp,
            &mut cpass,
            &self.scratch,
            &mut self.raw,
            &mut self.cache,
        );

        self.cleanup(&mut cpass, &stroke);

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);
    }

    pub fn discard(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("page_discard"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("page_discard"),
            timestamp_writes: None,
        });

        self.cleanup(&mut cpass, &stroke);

        drop(cpass);
        self.lp.queue.submit([encoder.finish()]);
    }

    fn cleanup(&mut self, cpass: &mut ComputePass, stroke: &Stroke) {
        if stroke.replace {
            for (key, chunk) in self.scratch.chunks.drain() {
                let chunk_rect = chunks_key_to_rect(key, self.scratch.chunk_size);
                self.lp.clear_chunk_direct(cpass, &chunk, chunk_rect);
                self.pool.list.push(chunk);
            }
        } else {
            for (key, chunk) in self.scratch.chunks.drain() {
                let chunk_rect = chunks_key_to_rect(key, self.scratch.chunk_size);
                self.lp.clear_chunk(cpass, &chunk, chunk_rect, stroke.dirty);
                self.pool.list.push(chunk);
            }
        }

        // TODO Evaluate raw chunks and turn suitable chunks back into associated/transparent
    }

    /// Copy the preset at `index` into the temporary working brush.
    ///
    /// The registry itself is never modified, so tuning the active brush is discarded once
    /// another preset is selected.
    pub fn select_brush(&mut self, index: usize) {
        self.active_brush = index;
        self.active = self.brushes[index].brush.dup();
        self.active.set_color(self.color);
    }

    pub fn color(&self) -> Srgba {
        self.active.color().unwrap_or(self.color)
    }

    pub fn set_color(&mut self, color: Srgba) {
        self.color = color;
        self.active.set_color(color);
    }

    pub fn pick_color(&mut self, cursor: I64Vec2, world: &World) {
        let cmd = world.commander("async_color_pick");
        self.lp
            .pick_color(&self.raw, cursor.q32_floor(), move |color| {
                cmd.queue(move |world| {
                    let mut wrapper = world.single_fetch_mut::<LayerPage>();
                    wrapper.set_color(color.into_color());
                    world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
                });
            });
    }

    fn process_stream(&mut self, world: &World) {
        while let Ok(output) = self.thread_rx.try_recv() {
            match output {
                ThreadOutput::ThreadDebugMessage(msg) => {
                    world.queue_trigger(world.single::<LayerPage>(), LayerDebugMessage(msg));
                }
                ThreadOutput::RawInsert(key, chunk) => {
                    self.raw.chunks.insert(key, chunk);
                }
                ThreadOutput::RawRemove(key) => {
                    self.raw.chunks.remove(&key);
                }
                ThreadOutput::CacheInsert(key, chunk) => {
                    self.cache.chunks.insert(key, chunk);
                }
                ThreadOutput::CacheRemove(key) => {
                    self.cache.chunks.remove(&key);
                }
            }
        }
    }

    fn render(&mut self, rpass: &mut RenderPass, extra: RenderExtra) {
        let (start, end) = extra.diagnosis.assign("main > layers");
        extra.diagnosis.write(rpass, start);

        let camera = extra.camera;
        let view_rect = camera.world_view_rect();
        let mipmap = zoom_to_mipmap(camera.zoom);
        let actual_mipmap = mipmap.min(self.cache.mipmap_cap.saturating_sub(1));
        let pixel = camera.zoom.q32_as_f64().exp2() > 4.0;

        match self.debug {
            false => rpass.set_pipeline(&self.lp.render_pipelines.over),
            true => rpass.set_pipeline(&self.lp.render_pipelines.debug),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        match pixel {
            true => rpass.set_bind_group(1, &self.lp.sampler_group_unfiltered, &[]),
            false => rpass.set_bind_group(1, &self.lp.sampler_group_filtered, &[]),
        }

        for key in rect_chunks_stack_layer_iter(view_rect, self.cache.chunk_size, actual_mipmap) {
            if let Some(CacheLayerChunk::Filled(chunk)) = self.cache.chunks.get(&key) {
                rpass.set_bind_group(2, &chunk.render, &[]);
                rpass.draw(0..4, 0..1);
            }
        }

        extra.diagnosis.write(rpass, end);
    }

    pub fn undo(&mut self) {
        if self.traveler.undo_available(&self.raw) {
            let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("page_undo"),
            });

            let stroke = self.traveler.undo(&mut encoder, &self.raw).unwrap();

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_undo"),
                timestamp_writes: None,
            });

            stroke.raw_merge(&self.lp, &mut cpass, &mut self.raw, &mut self.cache);

            drop(cpass);
            self.lp.queue.submit([encoder.finish()]);
        } else {
            log::debug!("failed to undo");
        }
    }

    pub fn redo(&mut self) {
        if self.traveler.redo_available(&self.raw) {
            let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("page_undo"),
            });

            let stroke = self.traveler.redo(&mut encoder, &self.raw).unwrap();

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_undo"),
                timestamp_writes: None,
            });

            stroke.raw_merge(&self.lp, &mut cpass, &mut self.raw, &mut self.cache);

            drop(cpass);
            self.lp.queue.submit([encoder.finish()]);
        } else {
            log::debug!("failed to redo");
        }
    }

    pub fn set_page(&mut self, page: u64) {
        self.traveler.clear();
        self.thread_tx.send(ThreadInput::SetPage(page)).unwrap();
    }
}

impl Drop for LayerPage {
    fn drop(&mut self) {
        self.thread_tx.send(ThreadInput::Abort).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Element for LayerPage {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.single::<LayerPage>();

        let save = world.insert(Autosave(Box::new(move |world, _write| {
            let this = world.single_fetch::<LayerPage>();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let main_camera = world.single_fetch::<MainCamera>().0;
        world.observer(main_camera, move |&CameraUpdated, world| {
            let this = world.single_fetch::<LayerPage>();
            let camera = world.fetch(main_camera);

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.size,
                    camera.center,
                ))
                .unwrap();
        });

        let control = world.insert(RenderControl {
            camera: Some(main_camera),
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this);
                this.process_stream(world);

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(move |world, rpass, extra| {
                let mut this = world.single_fetch_mut::<LayerPage>();
                this.render(rpass, extra);
            })),
        });

        RenderControl::reorder(main_camera, Some(-100), world, control);
        world.dependency(control, this);
    }
}
