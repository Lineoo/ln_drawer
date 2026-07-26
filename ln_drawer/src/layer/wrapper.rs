use std::{
    collections::VecDeque,
    sync::mpsc::channel,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use glam::{DVec2, IVec2, UVec2, Vec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, World};
use palette::Srgba;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Color, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, Device, Extent3d, FragmentState, LoadOp, Operations,
    Origin3d, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPass,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPassTimestampWrites, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    SurfaceConfiguration, TexelCopyTextureInfoBase, Texture, TextureAspect, TextureSampleType,
    TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
};
use winit::{
    event::{ElementState, PointerKind, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    layer::{
        Layer, LayerPipeline,
        brush::BrushPipeline,
        chunk_to_rect, create_chunk, create_chunk_texture,
        interpolate::Draw,
        modifier::Modifier,
        rect_to_chunks,
        stream::{StreamConfig, ThreadInput, ThreadOutput, loading_thread},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        MSAA_STATE, Render, RenderControl, RenderExtra, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged, CameraUtils, UICamera},
        rounded::{RoundedRect, RoundedRectDescriptor},
    },
    save::{Autosave, SaveDatabase},
    tools::{
        collider::ToolCollider,
        modifiers::ModifiersTool,
        pointer::{PointerHover, PointerHoverStatus},
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    widgets::{WidgetEnabled, WidgetRectangle},
};

const UNDO_LIMIT: usize = 32;
const MAIN_CHUNK_SIZE: u32 = 512;
const MAIN_CHUNK_MIPMAP: u8 = 8;

pub struct LayerDebugMessage(pub String);

pub struct LayerWrapper {
    pub main: Layer,
    pub brush: BrushPipeline,

    pub undos: VecDeque<Layer>,
    pub redos: Vec<Layer>,

    pub debug: bool,

    brush_preview: Handle<RoundedRect>,
    compositing_texture: Texture,
    compositing_config: SurfaceConfiguration,
    compositing_render_bind: BindGroup,

    present_pipeline: RenderPipeline,

    thread_tx: std::sync::mpsc::Sender<ThreadInput>,
    thread_rx: std::sync::mpsc::Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,
}

impl LayerWrapper {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        let brush = BrushPipeline::new(LayerPipeline::new(
            render.device.clone(),
            render.queue.clone(),
            render.config.format,
            &camera_bind.layout,
        ));

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();

        let (input_tx, input_rx) = channel();
        let (output_tx, output_rx) = channel();

        let stream_config = StreamConfig {
            database,
            device: render.device.clone(),
            queue: render.queue.clone(),
            chunk_render_layout: brush.layer.chunk_render_layout.clone(),
            chunk_draw_layout: brush.layer.chunk_draw_layout.clone(),
            chunk_size: MAIN_CHUNK_SIZE,
            mipmap_levels: MAIN_CHUNK_MIPMAP,
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
            loading_thread(stream_config, input_rx, output_tx).unwrap();
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

        let (compositing_texture, compositing_render_bind) =
            compositing_resources(&render.device, &render.config);

        let present_pipeline = present_pipeline(&render.device, &render.config);

        LayerWrapper {
            main: Layer {
                chunks: HashMap::new(),
                mipmap_levels: MAIN_CHUNK_MIPMAP,
                chunk_size: MAIN_CHUNK_SIZE,
                controlled: true,
            },
            undos: VecDeque::new(),
            redos: Vec::new(),
            brush,
            brush_preview,
            debug: false,
            compositing_texture,
            compositing_config: render.config.clone(),
            compositing_render_bind,
            present_pipeline,
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

        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            let WindowEvent::KeyboardInput { event, .. } = event else {
                return;
            };

            if event.physical_key == KeyCode::KeyZ
                && !event.repeat
                && event.state == ElementState::Pressed
            {
                let mut this = world.fetch_mut(this).unwrap();
                let modifier = world.single_fetch::<ModifiersTool>().unwrap();
                let ctrl = modifier.modifiers.state().control_key();
                let shift = modifier.modifiers.state().shift_key();
                if ctrl {
                    if !shift {
                        this.undo();
                    } else {
                        this.redo();
                    }

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    lnwindow.window.request_redraw();
                }
            }
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
                    && !this.brush.erase
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
                            this.undo_stock();
                            this.brush.submit_stream(&mut this.main, &this.thread_tx);
                            temp_erase_mode = Some(this.brush.modifier);
                            this.brush.erase = true;
                            this.brush.modifier = TEMP_ERASE_MODIFIER;
                            drag_start = None;
                        } else {
                            drag_start = None;
                        }
                    }
                }

                this.brush.paint(
                    &this.main,
                    Draw {
                        position: primary.position,
                        force: primary.data.force.unwrap_or(1.0),
                    },
                );

                this.brush.request_stream(&this.main, &this.thread_tx);

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                lnwindow.window.request_redraw();
            } else {
                let this = &mut *world.fetch_mut(this).unwrap();

                this.undo_stock();
                this.brush.submit_stream(&mut this.main, &this.thread_tx);

                if let Some(ori) = temp_erase_mode {
                    temp_erase_mode = None;
                    this.brush.erase = false;
                    this.brush.modifier = ori;
                }
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
        self.brush
            .layer
            .render(&self.main, &mut rpass, &camera, self.debug, false);
        extra.diagnosis.write(&mut rpass, end);

        let (start, end) = extra.diagnosis.assign("layers > scratch");
        extra.diagnosis.write(&mut rpass, start);
        self.brush.layer.render(
            &self.brush.scratch,
            &mut rpass,
            &camera,
            self.debug,
            self.brush.erase,
        );
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

    fn undo_stock(&mut self) {
        self.redos.clear();

        let mut to_backup = Vec::new();
        for &src_key in self.brush.scratch.chunks.keys() {
            let (start, end) = rect_to_chunks(
                chunk_to_rect(src_key, self.brush.scratch.chunk_size),
                0,
                self.main.chunk_size,
            );

            for x in start.0..end.0 {
                for y in start.1..end.1 {
                    let dst_key = (x, y, 0);

                    if !to_backup.contains(&dst_key) {
                        to_backup.push(dst_key);
                    }
                }
            }
        }

        let mut encoder =
            self.brush
                .layer
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("layer_backup"),
                });

        let mut backup_layer = Layer {
            chunks: HashMap::new(),
            chunk_size: MAIN_CHUNK_SIZE,
            mipmap_levels: 1,
            controlled: false,
        };
        for dst_key in to_backup {
            let Some(src_chunk) = self.main.chunks.get(&dst_key) else {
                continue;
            };
            let dst_texture =
                create_chunk_texture(&self.brush.layer.device, backup_layer.chunk_size);
            let dst_chunk = create_chunk(
                &self.brush.layer.device,
                backup_layer.chunk_size,
                &self.brush.layer.chunk_render_layout,
                &self.brush.layer.chunk_draw_layout,
                dst_texture,
                dst_key,
            );
            encoder.copy_texture_to_texture(
                TexelCopyTextureInfoBase {
                    texture: &src_chunk.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyTextureInfoBase {
                    texture: &dst_chunk.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                Extent3d {
                    width: backup_layer.chunk_size,
                    height: backup_layer.chunk_size,
                    depth_or_array_layers: 1,
                },
            );
            backup_layer.chunks.insert(dst_key, dst_chunk);
        }

        self.brush.layer.queue.submit([encoder.finish()]);
        self.undos.push_back(backup_layer);

        while self.undos.len() > UNDO_LIMIT {
            self.undos.pop_front();
        }
    }

    fn undo(&mut self) {
        let Some(mut backup) = self.undos.pop_back() else {
            return;
        };

        for key in backup.chunks.keys() {
            if !self.main.chunks.contains_key(key) {
                log::debug!("failed to undo");
                return;
            }
        }

        let mut redo_chunks = Vec::new();
        for (key, chunk) in backup.chunks.drain() {
            self.thread_tx
                .send(ThreadInput::SwapChunk(key, chunk))
                .unwrap();
            if let Some(old) = self.main.chunks.get(&key).cloned() {
                redo_chunks.push((key, old));
            }
        }

        for (key, chunk) in redo_chunks {
            backup.chunks.insert(key, chunk);
        }

        self.redos.push(backup);
    }

    fn redo(&mut self) {
        let Some(mut backup) = self.redos.pop() else {
            return;
        };

        for key in backup.chunks.keys() {
            if !self.main.chunks.contains_key(key) {
                log::debug!("failed to redo");
                return;
            }
        }

        let mut undo_chunks = Vec::new();
        for (key, chunk) in backup.chunks.drain() {
            self.thread_tx
                .send(ThreadInput::SwapChunk(key, chunk))
                .unwrap();
            if let Some(old) = self.main.chunks.get(&key).cloned() {
                undo_chunks.push((key, old));
            }
        }

        for (key, chunk) in undo_chunks {
            backup.chunks.insert(key, chunk);
        }

        self.undos.push_back(backup);
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
    let texture = device.create_texture(&Render::screen_texture(
        "compositing",
        &config,
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
    ));

    let layout = device.create_bind_group_layout(&LAYOUT_COMPOSITING_PRESENT);

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("compositing_render"),
        layout: &layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(
                &texture.create_view(&TextureViewDescriptor::default()),
            ),
        }],
    });

    (texture, bind_group)
}

fn present_pipeline(device: &Device, config: &SurfaceConfiguration) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("wrapper_present_shader"),
        source: ShaderSource::Wgsl(include_str!("present.wgsl").into()),
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
            draw: Some(Box::new(move |world, rpass, extra| {
                let mut this = world.single_fetch_mut::<LayerWrapper>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();
                this.render(&camera, rpass, extra);
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
