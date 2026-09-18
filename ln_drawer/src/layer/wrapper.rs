use std::{
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
};

use glam::{IVec2, UVec2, Vec4};
use hashbrown::HashMap;
use ln_world::{Element, Handle, World};
use palette::Srgba;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Color, ColorTargetState,
    ColorWrites, Device, Extent3d, FragmentState, LoadOp, Operations, PipelineLayoutDescriptor,
    PrimitiveState, PrimitiveTopology, RenderPass, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPassTimestampWrites, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, StoreOp, SurfaceConfiguration, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor,
    TextureViewDimension, VertexState,
};

use crate::{
    layer::{
        DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_ENABLED, Layer, LayerPipeline,
        brush::{
            BrushParams, Draw, DrawPipeline, blur::BlurBrush, param::BrushParam, pixel::PixelBrush,
            round::RoundBrush, smudge::SmudgeBrush, tint::TintBrush,
        },
        stream::{StreamConfig, ThreadInput, ThreadOutput, loading_thread},
        traveler::Traveler,
    },
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl, RenderExtra, RenderInformation,
        camera::{Camera, CameraBind, CameraUpdated, MainCamera, UICamera},
    },
    save::{Autosave, SaveDatabase},
    widgets::{renderer::rrect::RRect, shaders::shader_compile},
};

pub struct LayerDebugMessage(pub String);

pub const BRUSH_PREVIEW_SHADOW_BLUR: i32 = 30;

pub struct BrushConfigurationChanged;

/// A named brush stored in the preset registry.
pub struct BrushPreset {
    /// i18n key of the display name.
    pub label: &'static str,
    pub brush: Box<dyn BrushParams>,
}

pub struct LayerWrapper {
    pub main: Layer,
    pub brush: DrawPipeline,
    pub traveler: Traveler,

    /// Immutable registry of brush presets.
    pub brushes: Vec<BrushPreset>,
    /// Index of the preset the active brush was copied from.
    pub active_brush: usize,
    /// Temporary working copy of [`Self::brushes`]`[active_brush]`; edits never touch the registry.
    active: Box<dyn BrushParams>,
    color: Srgba,

    pub debug: bool,

    pub temp_erase: RoundBrush,

    pub brush_preview: Handle<RRect>,
    pub brush_preview_shadow: Handle<RRect>,
    pub compositing_texture: Texture,
    pub compositing_config: SurfaceConfiguration,
    pub compositing_render_bind: BindGroup,

    pub present_pipeline: RenderPipeline,

    pub thread_tx: Sender<ThreadInput>,
    pub thread_rx: Receiver<ThreadOutput>,
    pub thread: Option<JoinHandle<()>>,
}

impl LayerWrapper {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        let layer = Arc::new(LayerPipeline::new(
            render.adapter.clone(),
            render.device.clone(),
            render.queue.clone(),
            TextureFormat::Rgba8Unorm,
            &camera_bind.layout,
        ));

        let brush = DrawPipeline::new(layer.clone());
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

        let (compositing_texture, compositing_render_bind) =
            compositing_resources(&render.device, &render.config);

        let present_pipeline = present_pipeline(&render.device, &render.config);

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
                    color_ratio: BrushParam::constant(0.044),
                    sample_radius: BrushParam::constant(0.5),
                    sample_rate: BrushParam::constant(0.07),
                }),
            },
            BrushPreset {
                label: "brush.tint",
                brush: Box::new(TintBrush {
                    size: BrushParam::force_index(10.0, 30.0, 1.0),
                    flow: Vec4::new(0.05, 0.7, 0.7, 1.0),
                    softness: BrushParam::constant(0.5),
                    spacing: BrushParam::constant(0.1),
                    color,
                }),
            },
        ];
        let mut active = brushes[0].brush.dup();
        active.set_color(color);

        LayerWrapper {
            main: Layer {
                chunks: HashMap::new(),
                mipmap_levels: DEFAULT_MIPMAP_ENABLED,
                chunk_size: DEFAULT_CHUNK_SIZE,
                controlled: true,
            },
            brush,
            traveler,
            brushes,
            active_brush: 0,
            active,
            color,
            debug: false,
            temp_erase: RoundBrush {
                size: BrushParam::force_index(5.0, 15.0, 1.0),
                flow: BrushParam::force_index(0.9, 1.0, 0.5),
                softness: BrushParam::constant(0.5),
                spacing: BrushParam::constant(0.1),
                color: Srgba::new(1.0, 1.0, 1.0, 1.0),
                erase: true,
            },
            brush_preview,
            brush_preview_shadow,
            compositing_texture,
            compositing_config: render.config.clone(),
            compositing_render_bind,
            present_pipeline,
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

    /// Draw one segment with the temporary working brush.
    pub fn draw_active(&mut self, draw: Draw) {
        self.active.draw(&mut self.brush, &self.main, draw);
        self.brush.request_stream(&self.main, &self.thread_tx);
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
                    self.main.chunks.insert(key, chunk_bind);
                }
                ThreadOutput::Remove(key) => {
                    self.main.chunks.remove(&key);
                }
            }
        }
    }

    fn layers_render(&mut self, camera: &Camera, extra: &mut RenderExtra) {
        // prepare rpass
        let (start, end) = extra.diagnosis.assign("layers");
        let compositing_view = self
            .compositing_texture
            .create_view(&TextureViewDescriptor::default());
        let mut rpass = extra
            .early_encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("in_layer_rpass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &compositing_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: Some(RenderPassTimestampWrites {
                    query_set: extra.diagnosis.query,
                    beginning_of_pass_write_index: Some(start),
                    end_of_pass_write_index: Some(end),
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });

        // draw layers

        let (start, end) = extra.diagnosis.assign("layers > main");
        extra.diagnosis.write(&mut rpass, start);
        (self.brush.layer).render(&self.main, &mut rpass, &camera, self.debug, false);
        extra.diagnosis.write(&mut rpass, end);

        let (start, end) = extra.diagnosis.assign("layers > scratch");
        extra.diagnosis.write(&mut rpass, start);
        (self.brush).scratch_render(&mut rpass, &camera, self.debug);
        extra.diagnosis.write(&mut rpass, end);
    }

    fn render(&mut self, camera: &Camera, rpass: &mut RenderPass, mut extra: RenderExtra) {
        // compositing texture
        if self.compositing_config != *extra.surface_config {
            self.compositing_config = extra.surface_config.clone();
            (self.compositing_texture, self.compositing_render_bind) =
                compositing_resources(extra.device, extra.surface_config)
        }

        // in-layer render pass
        self.layers_render(camera, &mut extra);

        // final screen draw
        let (start, end) = extra.diagnosis.assign("main > layers_present");
        extra.diagnosis.write(rpass, start);

        rpass.set_pipeline(&self.present_pipeline);
        rpass.set_bind_group(0, &self.compositing_render_bind, &[]);
        rpass.draw(0..3, 0..1);

        extra.diagnosis.write(rpass, end);
    }

    pub fn stock(&mut self) {
        let Some(stroke) = &self.brush.stroke else {
            return;
        };

        self.traveler.stock(&self.main, stroke.dirty);
    }

    pub fn undo(&mut self) {
        if self.traveler.undo_available(&self.main) {
            let dirty = self.traveler.undo(&self.main).unwrap();
            self.brush.layer.generate_mipmaps(&self.main, dirty);
        } else {
            log::debug!("failed to undo");
        }
    }

    pub fn redo(&mut self) {
        if self.traveler.redo_available(&self.main) {
            let dirty = self.traveler.redo(&self.main).unwrap();
            self.brush.layer.generate_mipmaps(&self.main, dirty);
        } else {
            log::debug!("failed to redo");
        }
    }

    pub fn set_page(&mut self, page: u64) {
        self.traveler.clear();
        self.thread_tx.send(ThreadInput::SetPage(page)).unwrap();
    }
}

const LAYOUT_COMPOSITING_PRESENT: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("compositing_render"),
    entries: &[BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }],
};

fn compositing_resources(device: &Device, config: &SurfaceConfiguration) -> (Texture, BindGroup) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("compositing"),
        size: Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let layout = device.create_bind_group_layout(&LAYOUT_COMPOSITING_PRESENT);

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("compositing_render"),
        layout: &layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&texture.create_view(&Default::default())),
        }],
    });

    (texture, bind_group)
}

fn present_pipeline(device: &Device, config: &SurfaceConfiguration) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("wrapper_present_shader"),
        source: ShaderSource::Wgsl(shader_compile(include_str!("present.wgsl"), &[]).into()),
    });

    let compositing_render_layout = device.create_bind_group_layout(&LAYOUT_COMPOSITING_PRESENT);

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&compositing_render_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("wrapper_present_pipeline"),
        layout: Some(&layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format: config.format,
                blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        depth_stencil: None,
        multisample: MSAA_STATE,
        multiview_mask: None,
        cache: None,
    })
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

        let main_camera = world.single_fetch::<MainCamera>().unwrap().0;
        world.observer(main_camera, move |&CameraUpdated, world| {
            let this = world.single_fetch::<LayerWrapper>().unwrap();
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
                let mut this = world.single_fetch_mut::<LayerWrapper>().unwrap();
                let camera = world.fetch(main_camera).unwrap();
                this.render(&camera, rpass, extra);
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
