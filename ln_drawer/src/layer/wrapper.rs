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
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor, RenderPass};

use crate::{
    layer::{
        DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_DISABLED, DEFAULT_MIPMAP_ENABLED, Layer, LayerPipeline,
        brush::{
            BrushParams, Draw, DrawPipeline, blur::BlurBrush, param::BrushParam, pixel::PixelBrush,
            round::RoundBrush, smudge::SmudgeBrush, tint::TintBrush,
        },
        rect_to_chunks,
        stream::{StreamConfig, ThreadInput, ThreadOutput, loading_thread},
        traveler::Traveler,
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl, RenderExtra, RenderInformation,
        camera::{Camera, CameraBind, CameraUpdated, MainCamera, UICamera},
    },
    save::{Autosave, SaveDatabase},
    widgets::renderer::rrect::RRect,
};

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
    pub main: Layer,
    merge: Layer,

    pub draw: DrawPipeline,
    traveler: Traveler,

    /// Immutable registry of brush presets.
    pub brushes: Vec<BrushPreset>,
    /// Index of the preset the active brush was copied from.
    pub active_brush: usize,

    active: Box<dyn BrushParams>,
    erase: RoundBrush,
    color: Srgba,

    pub debug: bool,

    pub brush_preview: Handle<RRect>,
    pub brush_preview_shadow: Handle<RRect>,

    pub thread_tx: Sender<ThreadInput>,
    pub thread_rx: Receiver<ThreadOutput>,
    pub thread: Option<JoinHandle<()>>,
}

impl LayerPage {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        let layer = Arc::new(LayerPipeline::new(
            render.adapter.clone(),
            render.device.clone(),
            render.queue.clone(),
            render.config.format,
            &camera_bind.layout,
        ));

        let draw = DrawPipeline::new(layer.clone());
        let traveler = Traveler::new(layer.clone());

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();
        let window = world.single_fetch::<Lnwindow>().unwrap().window.clone();

        let (input_tx, input_rx) = channel();
        let (output_tx, output_rx) = channel();

        let stream_config = StreamConfig {
            database,
            device: render.device.clone(),
            queue: render.queue.clone(),
            page: 0,
            chunk_size: DEFAULT_CHUNK_SIZE,
            mipmap_levels: DEFAULT_MIPMAP_ENABLED,
            layer_pipeline: layer.clone(),
            window,
        };

        let main_camera = world.single_fetch::<MainCamera>().unwrap();
        let camera = world.fetch(main_camera.0).unwrap();
        input_tx
            .send(ThreadInput::SetStreamCamera(
                camera.zoom,
                camera.size,
                camera.center,
            ))
            .unwrap();

        let thread = std::thread::spawn(move || {
            loading_thread(stream_config, input_rx, output_tx).unwrap();
        });

        let ui_camera = world.single_fetch::<UICamera>().unwrap();
        let (brush_preview, brush_preview_shadow) = world.enter(ui_camera.0, || {
            let preview = world.insert(RRect {
                rect: Rectangle::new_half(IVec2::new(0, 0), UVec2::new(1, 1)),
                order: -10,
                color: Srgba::new(0.5, 0.5, 0.5, 0.4),
                radius: 0.5,
                width: 0.0,
                enabled: false,
            });
            let preview_shadow = world.insert(RRect {
                rect: Rectangle::new_half(IVec2::new(0, 0), UVec2::new(1, 1)),
                order: -11,
                color: Srgba::new(0.0, 0.0, 0.0, 0.3),
                radius: 0.5,
                width: BRUSH_PREVIEW_SHADOW_BLUR as f32,
                enabled: false,
            });
            (preview, preview_shadow)
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

        LayerPage {
            main: Layer {
                chunks: HashMap::new(),
                mipmap_levels: DEFAULT_MIPMAP_ENABLED,
                chunk_size: DEFAULT_CHUNK_SIZE,
                controlled: true,
            },
            merge: Layer {
                chunks: HashMap::new(),
                mipmap_levels: DEFAULT_MIPMAP_DISABLED,
                chunk_size: DEFAULT_CHUNK_SIZE,
                controlled: false,
            },
            draw,
            traveler,
            brushes,
            active_brush: 0,
            active,
            color,
            debug: false,
            erase: RoundBrush {
                size: BrushParam::force_index(5.0, 15.0, 1.0),
                flow: BrushParam::force_index(0.9, 1.0, 0.5),
                softness: BrushParam::constant(0.3),
                spacing: BrushParam::constant(0.1),
                color: Srgba::new(1.0, 1.0, 1.0, 1.0),
                erase: true,
            },
            brush_preview,
            brush_preview_shadow,
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
        self.active.draw(&mut self.draw, &self.main, draw);
        self.draw.request_stream(&self.main, &self.thread_tx);
        if let Some(stroke) = &self.draw.stroke
            && !stroke.replace
        {
            self.draw.cache_merge_scratch(&self.main, &mut self.merge);
        }
    }

    pub fn draw_erase(&mut self, draw: Draw) {
        self.erase.draw(&mut self.draw, &self.main, draw);
        self.draw.request_stream(&self.main, &self.thread_tx);
        if let Some(stroke) = &self.draw.stroke
            && !stroke.replace
        {
            self.draw.cache_merge_scratch(&self.main, &mut self.merge);
        }
    }

    pub fn submit(&mut self) {
        let Some(stroke) = &self.draw.stroke else {
            return;
        };

        let mut encoder =
            (self.draw.layer.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("page_submit"),
            });

        self.traveler.stock(&mut encoder, &self.main, stroke.dirty);

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("page_submit"),
            timestamp_writes: None,
        });

        self.draw
            .submit(&mut self.main, Some(&self.thread_tx), &mut cpass);
        self.draw.recycle_all(&mut self.merge, &mut cpass);

        drop(cpass);
        self.draw.layer.queue.submit([encoder.finish()]);
    }

    pub fn discard(&mut self) {
        let mut encoder =
            (self.draw.layer.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("page_discard"),
            });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("page_discard"),
            timestamp_writes: None,
        });

        self.draw.discard(&mut cpass);
        self.draw.recycle_all(&mut self.merge, &mut cpass);

        drop(cpass);
        self.draw.layer.queue.submit([encoder.finish()]);
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
        let cmd = world.commander();
        self.draw
            .layer
            .pick_color(&self.main, cursor.q32_floor(), move |color| {
                cmd.queue(move |world| {
                    let mut wrapper = world.single_fetch_mut::<LayerPage>().unwrap();
                    wrapper.set_color(color.into_color());
                    world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
                });
            });
    }

    fn process_stream(&mut self, world: &World) {
        while let Ok(output) = self.thread_rx.try_recv() {
            match output {
                ThreadOutput::ThreadDebugMessage(msg) => {
                    world.queue_trigger(
                        world.single::<LayerPage>().unwrap(),
                        LayerDebugMessage(msg),
                    );
                }
                ThreadOutput::Insert(key, chunk_bind) => {
                    self.main.chunks.insert(key, chunk_bind);
                }
                ThreadOutput::Remove(key) => {
                    self.main.chunks.remove(&key);
                }
            }
        }
    }

    fn render(&mut self, camera: &Camera, rpass: &mut RenderPass, extra: RenderExtra) {
        let (start, end) = extra.diagnosis.assign("main > layers");
        extra.diagnosis.write(rpass, start);

        let view_rect = camera.world_view_rect();
        let mipmap = (-camera.zoom).q32_floor().max(0) as u8;
        let actual_mipmap = mipmap.min(self.main.mipmap_levels.saturating_sub(1));
        let pixel = camera.zoom.q32_as_f64().exp2() > 4.0;

        match self.debug {
            false => rpass.set_pipeline(&self.draw.layer.render_pipelines.over),
            true => rpass.set_pipeline(&self.draw.layer.render_pipelines.debug),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        match pixel {
            true => rpass.set_bind_group(1, &self.draw.layer.sampler_group_unfiltered, &[]),
            false => rpass.set_bind_group(1, &self.draw.layer.sampler_group_filtered, &[]),
        }

        let (src, dst) = rect_to_chunks(view_rect, actual_mipmap, self.main.chunk_size);
        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                if let Some(merge) = self.merge.chunks.get(&(x, y, actual_mipmap)) {
                    rpass.set_bind_group(2, &merge.render, &[]);
                    rpass.draw(0..4, 0..1);
                } else if let Some(scratch) =
                    self.draw.scratch_dst.chunks.get(&(x, y, actual_mipmap))
                {
                    rpass.set_bind_group(2, &scratch.render, &[]);
                    rpass.draw(0..4, 0..1);
                } else if let Some(chunk) = self.main.chunks.get(&(x, y, actual_mipmap)) {
                    rpass.set_bind_group(2, &chunk.render, &[]);
                    rpass.draw(0..4, 0..1);
                }
            }
        }

        extra.diagnosis.write(rpass, end);
    }

    pub fn undo(&mut self) {
        if self.traveler.undo_available(&self.main) {
            let dirty = self.traveler.undo(&self.main).unwrap();

            let mut encoder =
                (self.draw.layer.device).create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("page_undo"),
                });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_undo"),
                timestamp_writes: None,
            });

            self.draw
                .layer
                .generate_mipmaps(&self.main, dirty, &mut cpass);

            drop(cpass);
            self.draw.layer.queue.submit([encoder.finish()]);
        } else {
            log::debug!("failed to undo");
        }
    }

    pub fn redo(&mut self) {
        if self.traveler.redo_available(&self.main) {
            let dirty = self.traveler.redo(&self.main).unwrap();

            let mut encoder =
                (self.draw.layer.device).create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("page_undo"),
                });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("page_undo"),
                timestamp_writes: None,
            });

            self.draw
                .layer
                .generate_mipmaps(&self.main, dirty, &mut cpass);

            drop(cpass);
            self.draw.layer.queue.submit([encoder.finish()]);
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
        world.single::<LayerPage>().unwrap();

        let save = world.insert(Autosave(Box::new(move |world, _write| {
            let this = world.single_fetch::<LayerPage>().unwrap();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let main_camera = world.single_fetch::<MainCamera>().unwrap().0;
        world.observer(main_camera, move |&CameraUpdated, world| {
            let this = world.single_fetch::<LayerPage>().unwrap();
            let camera = world.fetch(main_camera).unwrap();

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.size,
                    camera.center,
                ))
                .unwrap();
        });

        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this).unwrap();
                this.process_stream(world);

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(move |world, rpass, extra| {
                let mut this = world.single_fetch_mut::<LayerPage>().unwrap();
                let camera = world.fetch(main_camera).unwrap();
                this.render(&camera, rpass, extra);
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
