pub mod camera;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ln_world::{Element, Handle, World};
use wgpu::*;
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{
    layout::val::ValChanged,
    lnwin::Lnwindow,
    measures::ScissorRect,
    render::camera::{Camera, UICamera},
    widgets::common::WidgetCommon,
};

pub const MSAA_SAMPLE_COUNT: u32 = 1;
pub const MSAA_STATE: MultisampleState = MultisampleState {
    count: MSAA_SAMPLE_COUNT,
    mask: !0,
    alpha_to_coverage_enabled: false,
};

pub const TIMESTAMP_COUNT: u32 = 256;
pub const TIMESTAMP_BUFFER_SIZE: u64 = (TIMESTAMP_COUNT * 8) as u64;

pub struct Render {
    // wgpu surface
    pub surface: Surface<'static>,
    pub config: SurfaceConfiguration,

    // wgpu interface
    pub instance: Instance,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub cache: Option<PipelineCache>,

    // msaa
    msaa_texture: Option<Texture>,

    // render pass
    pub clear_color: Color,

    // render control
    preparing: bool,

    // time tracing
    time_avg_prepare: Option<Duration>,
    time_avg_draw: Option<Duration>,
    time_avg_frametime: Option<Duration>,
    last_redraw: Option<Instant>,

    // diagnosis
    pub timestamp_poll: bool,
    timestamp_resolver: Buffer,
    timestamp_mapper: Buffer,
    timestamp_query: QuerySet,
    timestamp_mapped: Arc<AtomicBool>,
}

type RenderPrepareCommand = Box<dyn FnMut(&World) -> Option<RenderInformation> + Send>;
type RenderDrawCommand = Box<dyn FnMut(&World, &mut RenderPass<'_>, RenderExtra<'_, '_>) + Send>;

/// prepare to render and give related information
pub struct RenderPrepareControl {
    pub prepare: RenderPrepareCommand,
}

pub struct RenderInformation {
    pub keep_redrawing: bool,
}

/// draw with given render pass
pub struct RenderDrawControl {
    pub common: WidgetCommon,
    pub order: isize,
    pub draw: RenderDrawCommand,
}

#[non_exhaustive]
pub struct RenderExtra<'a, 'b> {
    pub device: &'a Device,
    pub queue: &'a Queue,
    pub camera: &'a Camera,
    pub early_encoder: &'a mut CommandEncoder,
    pub surface_config: &'a SurfaceConfiguration,
    pub diagnosis: &'a mut RenderDiagnosis<'b>,
    pub scissor: ScissorRect,
}

pub struct RenderDiagnosis<'a> {
    pub query: &'a QuerySet,
    pub slots: Vec<((u32, u32), String)>,
    pub front: u32,
    pub enable: bool,
}

impl Render {
    pub async fn new(lnwindow: &Lnwindow) -> Render {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
            display: None,
        });

        let surface = instance.create_surface(lnwindow.window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .unwrap();

        log::debug!("wgpu adapter: {:?}", adapter.get_info());

        let ideal_features = Features::TIMESTAMP_QUERY
            | Features::TIMESTAMP_QUERY_INSIDE_PASSES
            | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | Features::PIPELINE_CACHE;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: adapter.features() & ideal_features,
                required_limits: Limits::defaults(),
                experimental_features: ExperimentalFeatures::disabled(),
                memory_hints: MemoryHints::MemoryUsage,
                trace: Trace::Off,
            })
            .await
            .unwrap();

        let size = lnwindow.window.surface_size();
        let config = Render::configuration(&surface, &adapter, size);
        surface.configure(&device, &config);

        let timestamp_resolver = device.create_buffer(&BufferDescriptor {
            label: Some("timestamp_buffer_resolver"),
            size: TIMESTAMP_BUFFER_SIZE,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let timestamp_mapper = device.create_buffer(&BufferDescriptor {
            label: Some("timestamp_buffer_mapper"),
            size: (TIMESTAMP_COUNT * 8) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let timestamp_query = device.create_query_set(&QuerySetDescriptor {
            label: Some("timestamp_query"),
            ty: QueryType::Timestamp,
            count: TIMESTAMP_COUNT,
        });

        Render {
            surface,
            config,
            instance,
            adapter,
            device,
            queue,
            cache: None,
            msaa_texture: None,
            clear_color: Color::WHITE,
            preparing: false,
            time_avg_prepare: None,
            time_avg_draw: None,
            time_avg_frametime: None,
            last_redraw: None,
            timestamp_poll: false,
            timestamp_resolver,
            timestamp_mapper,
            timestamp_query,
            timestamp_mapped: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn surface_recreate(&mut self, lnwindow: &Lnwindow) {
        self.surface = self
            .instance
            .create_surface(lnwindow.window.clone())
            .unwrap();
        let size = lnwindow.window.surface_size();
        self.config = Render::configuration(&self.surface, &self.adapter, size);
        self.surface.configure(&self.device, &self.config);
        self.msaa_texture = None;
    }

    pub fn surface_resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.msaa_texture = None;
    }

    fn msaa_texture(config: &SurfaceConfiguration) -> TextureDescriptor<'_> {
        assert!(
            MSAA_SAMPLE_COUNT > 1,
            "msaa texture should be created only when msaa sample count > 1"
        );
        TextureDescriptor {
            label: Some("render_msaa"),
            size: Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: MSAA_SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format: config.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TRANSIENT_ATTACHMENT,
            view_formats: &[],
        }
    }

    fn configuration(
        surface: &Surface,
        adapter: &Adapter,
        size: PhysicalSize<u32>,
    ) -> SurfaceConfiguration {
        let caps = surface.get_capabilities(&adapter);
        let format = *caps
            .formats
            .iter()
            .max_by_key(|&format| match format {
                TextureFormat::Rgba8UnormSrgb => 100,
                TextureFormat::Bgra8UnormSrgb => 90,
                _ if format.is_srgb() => 10,
                _ => 0,
            })
            .unwrap();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode: {
                let caps = &caps.present_modes;
                if caps.contains(&PresentMode::FifoRelaxed) {
                    PresentMode::FifoRelaxed
                } else if caps.contains(&PresentMode::Fifo) {
                    PresentMode::Fifo
                } else {
                    *caps.first().unwrap()
                }
            },
            alpha_mode: {
                let caps = &caps.alpha_modes;
                if caps.contains(&CompositeAlphaMode::PreMultiplied) {
                    CompositeAlphaMode::PreMultiplied
                } else if caps.contains(&CompositeAlphaMode::PostMultiplied) {
                    CompositeAlphaMode::PostMultiplied
                } else if caps.contains(&CompositeAlphaMode::Inherit) {
                    CompositeAlphaMode::Inherit
                } else {
                    *caps.first().unwrap()
                }
            },
            color_space: SurfaceColorSpace::Auto,
            view_formats: vec![],
        };

        log::debug!("resize in {}, {}", config.width, config.height);
        log::debug!("texture format {:?}", config.format);
        log::debug!("present mode {:?} is selected", config.present_mode);
        log::debug!("alpha mode {:?} is selected", config.alpha_mode);

        config
    }

    pub fn draw(camera: &Camera, world: &World, rpass: &mut RenderPass, extra: RenderExtra) {
        for &control in &camera.renderers {
            let extra = RenderExtra {
                device: extra.device,
                queue: extra.queue,
                camera,
                early_encoder: extra.early_encoder,
                surface_config: extra.surface_config,
                diagnosis: extra.diagnosis,
                scissor: extra.scissor,
            };
            let mut control = world.fetch_mut(control);
            let visible = world.fetch(control.common.visible);
            if visible.val {
                (control.draw)(world, rpass, extra);
            }
        }
    }

    fn render_main(world: &mut World) {
        let timestamp_prepare = Instant::now();

        let mut render = world.single_fetch_mut::<Render>();
        render.preparing = true;
        drop(render);

        let mut refreshing = false;
        world.foreach_fetch_mut::<RenderPrepareControl>(|mut control| {
            if let Some(info) = (control.prepare)(world) {
                refreshing |= info.keep_redrawing;
            };
        });

        world.queue_cache::<RenderDrawControl>();
        world.queue_cache::<RenderPrepareControl>();

        world.flush();

        let mut render = world.single_fetch_mut::<Render>();
        render.preparing = false;
        drop(render);

        if refreshing {
            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        }

        let timestamp_draw = Instant::now();

        let render = &mut *world.single_fetch_mut::<Render>();

        // setup render pass

        let surface = match render.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Timeout => return,
            CurrentSurfaceTexture::Occluded => return,
            CurrentSurfaceTexture::Outdated => {
                render.surface.configure(&render.device, &render.config);
                return;
            }
            // TODO not correctly handled
            CurrentSurfaceTexture::Lost => return,
            CurrentSurfaceTexture::Validation => return,
        };

        let surface_texture = &surface.texture;
        let surface_view = surface_texture.create_view(&TextureViewDescriptor::default());

        let mut early_encoder = render
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("early_encoder"),
            });

        let mut encoder = render
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("main_encoder"),
            });

        let mut rpass = if MSAA_SAMPLE_COUNT > 1 {
            msaa_rpass(render, &surface_view, &mut encoder)
        } else {
            plain_rpass(render, &surface_view, &mut encoder)
        };

        let mut diagnosis = RenderDiagnosis {
            query: &render.timestamp_query,
            slots: vec![((0, 1), "main".into())],
            front: 2,
            enable: render.timestamp_poll,
        };

        // redraw

        let ui_camera = world.single_fetch::<UICamera>().0;
        let camera = &mut *world.fetch_mut(ui_camera);
        Render::draw(
            camera,
            world,
            &mut rpass,
            RenderExtra {
                device: &render.device,
                queue: &render.queue,
                camera: &camera,
                early_encoder: &mut early_encoder,
                surface_config: &render.config,
                diagnosis: &mut diagnosis,
                scissor: ScissorRect {
                    x: 0,
                    y: 0,
                    width: camera.surface_size.x,
                    height: camera.surface_size.y,
                },
            },
        );

        drop(rpass);

        // GPU timestamp resolve

        let timestamp_mapped = render.timestamp_mapped.load(Ordering::Acquire);
        if render.timestamp_poll && !timestamp_mapped {
            encoder.resolve_query_set(
                &render.timestamp_query,
                0..TIMESTAMP_COUNT,
                &render.timestamp_resolver,
                0,
            );
            encoder.copy_buffer_to_buffer(
                &render.timestamp_resolver,
                0,
                &render.timestamp_mapper,
                0,
                TIMESTAMP_BUFFER_SIZE,
            );
        }

        // submission

        let commands = [early_encoder.finish(), encoder.finish()];
        render.queue.submit(commands);
        render.queue.present(surface);

        // CPU + GPU profiling

        let timestamp_profiling = Instant::now();

        if render.timestamp_poll {
            let cpu_frametime = render
                .last_redraw
                .map(|last| timestamp_prepare.duration_since(last));
            let cpu_prepare_time = timestamp_draw.duration_since(timestamp_prepare);
            let cpu_draw_time = timestamp_profiling.duration_since(timestamp_draw);

            if let Some(cpu_frametime) = cpu_frametime {
                if let Some(prev_frametime) = &mut render.time_avg_frametime {
                    *prev_frametime = (*prev_frametime + cpu_frametime) / 2
                } else {
                    render.time_avg_frametime = Some(cpu_frametime);
                }
            }

            if let Some(prev_prepare) = &mut render.time_avg_prepare {
                *prev_prepare = (*prev_prepare + cpu_prepare_time) / 2
            } else {
                render.time_avg_prepare = Some(cpu_prepare_time);
            }

            if let Some(prev_draw) = &mut render.time_avg_draw {
                *prev_draw = (*prev_draw + cpu_draw_time) / 2
            } else {
                render.time_avg_draw = Some(cpu_draw_time);
            }
        }

        if render.timestamp_poll && !timestamp_mapped {
            let mapper = render.timestamp_mapper.clone();
            let mapped = render.timestamp_mapped.clone();
            let cmd = world.commander("render_timestamp_poll");
            let target = world.single::<Render>();
            let slots = std::mem::take(&mut diagnosis.slots);
            let period = render.queue.get_timestamp_period() as u64;
            let cpu_frametime = render.time_avg_frametime;
            let cpu_prepare_time = render.time_avg_prepare;
            let cpu_draw_time = render.time_avg_draw;

            render.timestamp_mapped.store(true, Ordering::Release);
            render
                .timestamp_mapper
                .map_async(MapMode::Read, .., move |state| {
                    if state.is_err() {
                        mapped.store(false, Ordering::Release);
                        return;
                    }

                    let view = mapper.get_mapped_range(..).unwrap();
                    let (chunks, _) = view.as_chunks::<8>();
                    let mut timestamps = [0u64; TIMESTAMP_COUNT as usize];
                    for (i, &chunk) in chunks.iter().enumerate() {
                        timestamps[i] = u64::from_le_bytes(chunk) * period;
                    }
                    drop(view);
                    mapper.unmap();
                    mapped.store(false, Ordering::Release);

                    let mut pairs = indexmap::IndexMap::<String, Duration>::new();
                    for ((start, end), name) in slots {
                        let duration = pairs.entry(name).or_default();
                        *duration += Duration::from_nanos(
                            timestamps[end as usize] - timestamps[start as usize],
                        );
                    }

                    pairs.sort_unstable_keys();

                    let cpu_profiling_time = timestamp_profiling.elapsed();

                    let mut output = String::new();

                    if let Some(cpu_frame) = cpu_frametime {
                        output += &format!(
                            "CPU frame time: {:>7.2}ms / {:>7.2} FPS\n",
                            cpu_frame.as_secs_f64() * (1e3f64),
                            1. / cpu_frame.as_secs_f32()
                        );
                    };

                    if let Some(cpu_prepare_time) = cpu_prepare_time {
                        output += &format!(
                            "CPU prepare time: {:>7.2} ms\n",
                            cpu_prepare_time.as_secs_f64() * (1e3f64)
                        );
                    }

                    if let Some(cpu_draw_time) = cpu_draw_time {
                        output += &format!(
                            "CPU draw time: {:>7.2} ms\n",
                            cpu_draw_time.as_secs_f64() * (1e3f64)
                        );
                    }

                    output += &format!(
                        "CPU profiling time: {:>7.2} ms\n",
                        cpu_profiling_time.as_secs_f64() * (1e3f64)
                    );

                    output += "\n";
                    output += &format!("GPU timestamp period: {} ns\n", period);

                    for (name, duration) in pairs {
                        let milis = duration.as_secs_f64() * (1e3f64);
                        output += &format!("{name:<30}{milis:>6.3} ms\n");
                    }

                    cmd.queue(move |world| {
                        world.trigger(target, &output);
                    });
                });
        }

        render.last_redraw = Some(timestamp_prepare);
    }
}

fn msaa_rpass<'encoder>(
    render: &mut Render,
    surface_view: &TextureView,
    encoder: &'encoder mut CommandEncoder,
) -> RenderPass<'encoder> {
    // prepare msaa texture if not exist
    let msaa_texture = render.msaa_texture.get_or_insert_with(|| {
        let desc = Render::msaa_texture(&render.config);
        render.device.create_texture(&desc)
    });

    let msaa_view = msaa_texture.create_view(&TextureViewDescriptor::default());

    encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("main_rpass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &msaa_view,
            resolve_target: Some(surface_view),
            ops: Operations {
                load: LoadOp::Clear(render.clear_color),
                store: StoreOp::Discard,
            },
            depth_slice: None,
        })],
        timestamp_writes: render.timestamp_poll.then(|| RenderPassTimestampWrites {
            query_set: &render.timestamp_query,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }),
        ..Default::default()
    })
}

fn plain_rpass<'encoder>(
    render: &mut Render,
    surface_view: &TextureView,
    encoder: &'encoder mut CommandEncoder,
) -> RenderPass<'encoder> {
    encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("main_rpass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: surface_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(render.clear_color),
                store: StoreOp::Store,
            },
            depth_slice: None,
        })],
        timestamp_writes: render.timestamp_poll.then(|| RenderPassTimestampWrites {
            query_set: &render.timestamp_query,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }),
        ..Default::default()
    })
}

impl RenderDiagnosis<'_> {
    pub fn assign(&mut self, name: &str) -> (u32, u32) {
        self.assign_string(name.into())
    }

    pub fn assign_string(&mut self, name: String) -> (u32, u32) {
        assert!(self.front + 1 < TIMESTAMP_COUNT, "too many timestamps");

        if self.enable {
            let pair = (self.front, self.front + 1);
            self.slots.push((pair, name));
            self.front += 2;
            (pair.0, pair.1)
        } else {
            let pair = (0, 0);
            pair
        }
    }

    pub fn write(&mut self, rpass: &mut RenderPass, index: u32) {
        if self.enable {
            rpass.write_timestamp(&self.query, index);
        }
    }
}

impl Element for Render {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();
        world.observer(lnwindow, move |event: &WindowEvent, world| match event {
            WindowEvent::SurfaceResized(size) => {
                let mut render = world.fetch_mut(this);
                render.surface_resize(*size);
            }

            WindowEvent::RedrawRequested => {
                world.queue(|world| Render::render_main(world));
            }

            _ => (),
        });

        #[cfg(target_os = "android")]
        if self.device.features().contains(Features::PIPELINE_CACHE) {
            use wgpu::util::pipeline_cache_key;

            use crate::save::{Autosave, Save, get_file_path};

            let filename = pipeline_cache_key(&self.adapter.get_info());
            let path = get_file_path(world, &filename.unwrap());

            let save = world.single_fetch::<Save>();
            let data = match save.sn().compact_on_startup {
                // drop previous cache so we could start over
                true => None,
                false => std::fs::read(path).ok(),
            };

            // SAFETY: NOT safe. The cache is on users' device, we cannot determine that by ourselves.
            self.cache = Some(unsafe {
                self.device.create_pipeline_cache(&PipelineCacheDescriptor {
                    label: Some("main_cache"),
                    data: data.as_ref().map(|vec| &vec[..]),
                    fallback: true,
                })
            });

            world.insert(Autosave(Box::new(move |world, _| {
                let this = world.fetch(this);
                let cache = this.cache.as_ref().unwrap();
                if let Some(data) = cache.get_data() {
                    let filename = pipeline_cache_key(&this.adapter.get_info());
                    let path = get_file_path(world, &filename.unwrap());
                    if let Err(err) = std::fs::write(path, data) {
                        log::error!("failed to write pipeline cache: {err}");
                    }
                }
            })));
        }
    }
}

impl Element for RenderDrawControl {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("render_control");
        world.set_autodeps(Some(this));

        let mut camera = world.fetch_mut(self.common.camera);
        camera.renderers.push(this);
        camera
            .renderers
            .sort_by_key(|&control| match control == this {
                true => self.order,
                false => world.fetch(control).order,
            });
        let lnwindow = world.single_fetch::<Lnwindow>();
        lnwindow.window.request_redraw();
        world.observer(self.common.visible, move |&ValChanged, world| {
            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        });

        world.dependency(this, self.common.visible);
        world.dependency(this, self.common.camera);
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let mut camera = world.fetch_mut(self.common.camera);
        camera.renderers.retain(|&x| x != this);
        let lnwindow = world.single_fetch::<Lnwindow>();
        lnwindow.window.request_redraw();
    }
}

impl Element for RenderPrepareControl {}
