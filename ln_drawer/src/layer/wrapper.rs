use std::{
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
};

use glam::{DVec2, I64Vec2, IVec2, UVec2, Vec4};
use hashbrown::HashMap;
use ln_world::{Element, Handle, World};
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor, RenderPass};

use crate::{
    layer::{
        CacheLayerChunk, CacheLayers, ChunkPool, DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_CAP,
        LayerPipeline, LayerRenderer, PlainLayer, RawLayer, Standalone,
        brush::{
            BrushParams, Draw, Stroke, blur::BlurBrush, param::BrushParam, pixel::PixelBrush,
            round::RoundBrush, smudge::SmudgeBrush, tint::TintBrush,
        },
        input::LayerInput,
        rect_chunks_stack_layer_iter,
        stream::{StreamConfig, StreamThread, ThreadInput, ThreadOutput},
        traveler::Traveler,
        zoom_to_mipmap,
    },
    layout::val::Val,
    lnwin::Lnwindow,
    measures::{FI64Ext, IRect},
    render::{
        Render, RenderDrawControl, RenderExtra, RenderPrepareControl,
        camera::{
            CameraBind, CameraDescriptor, CameraUtils, PageCamera, PageCameraArchive,
            PageCameraUtils, SetCameraOutdated, UICamera,
        },
    },
    save::{Autosave, Save, object::Objects},
    widgets::{
        common::WidgetCommon,
        container::{Container, ContainerAttr},
        renderer::rrect::RRect,
    },
};

const BRIDGE_CHUNK_SIZE: UVec2 = UVec2::splat(1024);
const SCRATCH_POOL_CAP: usize = 32;
const DEFAULT_ZOOM_OFFSET: f64 = 0.0;

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
    // TODO: move to ECS after world-multithread support
    pub lp: Arc<LayerPipeline>,
    rd: LayerRenderer,

    common: WidgetCommon,
    rect: Handle<Val<IRect>>,

    cache: CacheLayers,
    raw: RawLayer,
    scratch: PlainLayer,
    swap: Option<PlainLayer>,
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
    color: Vec4,

    pub debug: bool,

    pub cursor_rect: Handle<Val<IRect>>,
    pub cursor_shadow_rect: Handle<Val<IRect>>,
    pub cursor_visible: Handle<Val<bool>>,
    pub zoom_offset: f64,

    thread_tx: Sender<ThreadInput>,
    thread_rx: Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,
}

impl LayerPage {
    pub fn new(world: &World, common: WidgetCommon, rect: Handle<Val<IRect>>) -> Self {
        let render = world.single_fetch::<Render>();
        let camera_bind = world.single_fetch::<CameraBind>();

        let lp = Arc::new(LayerPipeline::new(
            render.adapter.clone(),
            render.device.clone(),
            render.queue.clone(),
            render.cache.as_ref(),
            render.target_format(),
            &camera_bind.layout,
        ));

        let rd = lp.create_render(Vec4::ONE);

        let traveler = Traveler::new(lp.clone());

        let database = world.single_fetch::<Save>().db();
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

        let thread = std::thread::spawn(move || {
            StreamThread::new(stream_config, input_rx, output_tx)
                .run()
                .unwrap();
        });

        let ui_camera = world.single_fetch::<UICamera>().0;

        let cursor_visible = world.insert(Val::new(false));
        let cursor_rect = world.insert(Val::new(IRect::new_half(
            IVec2::new(0, 0),
            UVec2::new(1, 1),
        )));
        let cursor_shadow_rect = world.insert(Val::new(IRect::new_half(
            IVec2::new(0, 0),
            UVec2::new(1, 1),
        )));
        let cursor_common = WidgetCommon {
            camera: ui_camera,
            visible: cursor_visible,
        };
        world.insert(RRect {
            common: cursor_common,
            rect: cursor_rect,
            order: 110,
            color: world.insert(Val::new(Vec4::new(0.5, 0.5, 0.5, 1.0) * 0.4)),
            radius: 0.5,
            width: 0.0,
        });
        world.insert(RRect {
            common: cursor_common,
            rect: cursor_shadow_rect,
            order: 109,
            color: world.insert(Val::new(Vec4::new(0.0, 0.0, 0.0, 1.0) * 0.3)),
            radius: 0.5,
            width: BRUSH_PREVIEW_SHADOW_BLUR as f32,
        });

        let color = Vec4::new(0.0, 0.0, 0.0, 1.0);

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

        let bridge = lp.create_standalone(IRect {
            origin: IVec2::ZERO,
            extend: BRIDGE_CHUNK_SIZE,
        });

        let support_read_write = lp.chunk_layout.read_write.is_some();

        LayerPage {
            lp,
            rd,
            common,
            rect,
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
            swap: (!support_read_write).then(|| PlainLayer {
                chunks: HashMap::new(),
                chunk_size: DEFAULT_CHUNK_SIZE,
            }),
            bridge,
            pool: ChunkPool {
                list: Vec::new(),
                stage: Vec::new(),
                cap: SCRATCH_POOL_CAP,
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
                color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                erase: true,
            },
            color,
            debug: false,
            cursor_rect,
            cursor_shadow_rect,
            cursor_visible,
            zoom_offset: DEFAULT_ZOOM_OFFSET,
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
        Self::draw_brush(
            &self.lp,
            &mut self.scratch,
            &mut self.swap,
            &mut self.raw,
            &mut self.cache,
            &mut self.bridge,
            &mut self.pool,
            &*self.active,
            &mut self.stroke,
            draw,
        )
    }

    pub fn draw_erase(&mut self, draw: Draw) {
        Self::draw_brush(
            &self.lp,
            &mut self.scratch,
            &mut self.swap,
            &mut self.raw,
            &mut self.cache,
            &mut self.bridge,
            &mut self.pool,
            &self.erase,
            &mut self.stroke,
            draw,
        )
    }

    fn draw_brush<T: BrushParams + ?Sized>(
        lp: &LayerPipeline,
        scratch: &mut PlainLayer,
        swap: &mut Option<PlainLayer>,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
        bridge: &mut Standalone,
        pool: &mut ChunkPool,
        brush: &T,
        stroke: &mut Option<Stroke>,
        draw: Draw,
    ) {
        let stroke = stroke.get_or_insert_with(|| Stroke::new(brush));

        let Some(batch) = stroke.batch(brush, draw) else {
            return;
        };

        if !batch.satisfied(raw, cache) {
            return;
        }

        stroke.confirm_batch(&batch);
        batch.draw(lp, scratch, swap, raw, cache, bridge, pool, brush);
    }

    pub fn submit(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("page_traveler_stock"),
        });

        self.traveler.stock(
            &mut encoder,
            &mut self.raw,
            &self.cache,
            &mut self.pool,
            stroke.dirty,
        );

        self.lp.queue.submit([encoder.finish()]);

        stroke.submit(
            &self.lp,
            &mut self.scratch,
            self.swap.as_mut(),
            &mut self.raw,
            &mut self.cache,
            &mut self.pool,
        );
    }

    pub fn discard(&mut self) {
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        stroke.discard(
            &self.lp,
            &mut self.scratch,
            self.swap.as_mut(),
            &mut self.raw,
            &mut self.cache,
            &mut self.pool,
        );
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

    pub fn color(&self) -> Vec4 {
        self.active.color().unwrap_or(self.color)
    }

    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
        self.active.set_color(color);
    }

    pub fn pick_color(&mut self, cursor: I64Vec2, world: &World) {
        let cmd = world.commander();
        self.lp
            .pick_color(&self.raw, &self.cache, cursor.q32_floor(), move |color| {
                cmd.queue(move |world| {
                    let mut wrapper = world.single_fetch_mut::<LayerPage>();
                    wrapper.set_color(color.with_w(1.0));
                    world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
                });
            });
    }

    fn process_stream(&mut self, world: &World) {
        let mut cache_ack = Vec::new();
        let mut raw_ack = Vec::new();
        while let Ok(output) = self.thread_rx.try_recv() {
            match output {
                ThreadOutput::ThreadDebugMessage(msg) => {
                    world.queue_trigger(world.single::<LayerPage>(), LayerDebugMessage(msg));
                }
                ThreadOutput::RawInsert(key, chunk) => {
                    self.raw.chunks.insert(key, chunk);
                }
                ThreadOutput::RawRemoveAsk(key) => {
                    self.raw.chunks.remove(&key);
                    raw_ack.push(key);
                }
                ThreadOutput::CacheInsert(key, chunk) => {
                    self.cache.chunks.insert(key, chunk);
                }
                ThreadOutput::CacheRemoveAsk(key) => {
                    self.cache.chunks.remove(&key);
                    cache_ack.push(key);
                }
            }
        }
        if !cache_ack.is_empty() {
            self.thread_tx
                .send(ThreadInput::CacheRemoveAck(cache_ack))
                .unwrap();
        }
        if !raw_ack.is_empty() {
            self.thread_tx
                .send(ThreadInput::RawRemoveAck(raw_ack))
                .unwrap();
        }
    }

    fn render(&mut self, rpass: &mut RenderPass, extra: RenderExtra) {
        let (start, end) = extra.diagnosis.assign("main > layers");
        extra.diagnosis.write(rpass, start);

        let camera = extra.camera;
        let view_rect = camera.world_view_rect();
        let mipmap = zoom_to_mipmap(camera.zoom + i64::q32_from_f64(self.zoom_offset))
            .min(self.cache.mipmap_cap.saturating_sub(1));
        let pixel = camera.zoom.q32_as_f64().exp2() > 4.0;

        match self.debug {
            false => rpass.set_pipeline(&self.lp.render_pipelines.main),
            true => rpass.set_pipeline(&self.lp.render_pipelines.debug),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        match pixel {
            true => rpass.set_bind_group(1, &self.rd.render_group_unfiltered, &[]),
            false => rpass.set_bind_group(1, &self.rd.render_group_filtered, &[]),
        }

        for key in rect_chunks_stack_layer_iter(view_rect, self.cache.chunk_size, mipmap) {
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

            let stroke = self
                .traveler
                .undo(&mut encoder, &mut self.raw, &self.cache, &mut self.pool)
                .unwrap();

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_undo"),
                timestamp_writes: None,
            });

            stroke.cache_generate_without_scratch(
                &self.lp,
                &mut cpass,
                &mut self.raw,
                &mut self.cache,
                &mut self.pool,
            );
            self.lp
                .cache_mipmaps(&mut cpass, &mut self.cache, &mut self.pool, stroke.dirty);
            stroke.cache_recycle_raw(&mut self.raw);

            drop(cpass);
            self.lp.queue.submit([encoder.finish()]);
        } else {
            log::debug!("failed to undo");
        }
    }

    pub fn redo(&mut self) {
        if self.traveler.redo_available(&self.raw) {
            let mut encoder = (self.lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("page_redo"),
            });

            let stroke = self
                .traveler
                .redo(&mut encoder, &mut self.raw, &self.cache, &mut self.pool)
                .unwrap();

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_redo"),
                timestamp_writes: None,
            });

            stroke.cache_generate_without_scratch(
                &self.lp,
                &mut cpass,
                &mut self.raw,
                &mut self.cache,
                &mut self.pool,
            );
            self.lp
                .cache_mipmaps(&mut cpass, &mut self.cache, &mut self.pool, stroke.dirty);
            stroke.cache_recycle_raw(&mut self.raw);

            drop(cpass);
            self.lp.queue.submit([encoder.finish()]);
        } else {
            log::debug!("failed to redo");
        }
    }

    pub fn set_page(&mut self, page: u64) {
        self.traveler.clear();
        self.cache.chunks.clear();
        self.raw.chunks.clear();
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
        world.scope("layer_page");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let save = world.single_fetch::<Save>();

        // Camera //

        let page_camera = world.build(CameraDescriptor {
            center: save.sn().page_camera.center,
            zoom: save.sn().page_camera.zoom,
        });
        let page_camera_utils = CameraUtils::new(&page_camera);

        self.thread_tx
            .send(ThreadInput::SetStreamCamera(
                page_camera.zoom,
                page_camera.surface_size,
                page_camera.center,
            ))
            .unwrap();

        let page_camera = world.insert(page_camera);
        let page_camera_utils = world.insert(page_camera_utils);

        world.insert(PageCamera(page_camera));
        world.insert(PageCameraUtils(page_camera_utils));

        world.insert(Autosave(Box::new(move |world, task| {
            let camera = world.fetch(page_camera);
            task.sn_doc.set_page_camera(PageCameraArchive {
                center: camera.center,
                zoom: camera.zoom,
            });
        })));

        // Container //

        world.insert(Container {
            common: self.common,
            order: -100,
            child: page_camera,
            rect: self.rect,
            attr: ContainerAttr {
                anchor: DVec2::new(0.5, 0.5),
                top: true,
                ..Default::default()
            },
        });

        let common = WidgetCommon {
            camera: page_camera,
            visible: self.common.visible,
        };

        // Thread //

        world.insert(Autosave(Box::new(move |world, _write| {
            let this = world.single_fetch::<LayerPage>();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.observer(page_camera, move |&SetCameraOutdated, world| {
            let this = world.single_fetch::<LayerPage>();
            let camera = world.fetch(page_camera);

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.surface_size,
                    camera.center,
                ))
                .unwrap();
        });

        // Interaction //

        world.insert(LayerInput::default());
        world.insert(Objects { common });

        // Render //

        world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let this = &mut *world.fetch_mut(this);
                this.process_stream(world);
                None
            }),
        });

        world.insert(RenderDrawControl {
            common,
            order: 0,
            draw: Box::new(move |world, rpass, extra| {
                let mut this = world.single_fetch_mut::<LayerPage>();
                this.render(rpass, extra);
            }),
        });
    }
}
