pub mod camera;
pub mod rectangle;
pub mod rounded;
pub mod vertex;

use std::time::{Duration, Instant};

use ln_world::{Element, Handle, World};
use wgpu::{
    Adapter, Buffer, BufferDescriptor, BufferUsages, Color, CommandEncoder,
    CommandEncoderDescriptor, CompositeAlphaMode, CurrentSurfaceTexture, Device, DeviceDescriptor,
    ExperimentalFeatures, Extent3d, Features, Instance, Limits, LoadOp, MapMode, MemoryHints,
    MultisampleState, Operations, PollType, PowerPreference, PresentMode, QuerySet,
    QuerySetDescriptor, QueryType, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPassTimestampWrites, RequestAdapterOptions, StoreOp, Surface,
    SurfaceColorSpace, SurfaceConfiguration, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, Trace,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{lnwin::Lnwindow, render::camera::Camera};

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

    // msaa
    msaa_texture: Option<Texture>,

    // render pass
    pub clear_color: Color,

    // render control
    preparing: bool,
    seq_dirty: Vec<(Handle<RenderControl>, Handle, isize)>,
    seq_remove: Vec<Handle<RenderControl>>,
    sequence: Vec<(Handle<RenderControl>, Handle, isize)>,

    // time tracing
    last_redraw: Option<Instant>,

    // diagnosis
    pub timestamp_poll: bool,
    pub timestamp_resolver: Buffer,
    pub timestamp_mapper: Buffer,
    pub timestamp_query: QuerySet,
}

type RenderPrepareCommand = Box<dyn FnMut(&World) -> Option<RenderInformation>>;
type RenderDrawCommand = Box<dyn FnMut(&World, &mut RenderPass<'_>, RenderExtra<'_, '_>)>;

/// Need to call `RenderControl::reorder` before it can render normally.
pub struct RenderControl {
    /// prepare to render and give related information
    pub prepare: Option<RenderPrepareCommand>,

    /// draw with given render pass
    pub draw: Option<RenderDrawCommand>,
}

pub struct RenderInformation {
    pub keep_redrawing: bool,
}

#[non_exhaustive]
pub struct RenderExtra<'a, 'b> {
    pub device: &'a Device,
    pub queue: &'a Queue,
    pub early_encoder: &'a mut CommandEncoder,
    pub surface_config: &'a SurfaceConfiguration,
    pub diagnosis: &'a mut RenderDiagnosis<'b>,
}

pub struct RenderDiagnosis<'a> {
    pub query: &'a QuerySet,
    pub slots: Vec<((usize, usize), String)>,
    pub front: usize,
}

impl Render {
    pub async fn new(lnwindow: &Lnwindow) -> Render {
        let instance = Instance::default();

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

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | Features::TIMESTAMP_QUERY
                    | Features::TIMESTAMP_QUERY_INSIDE_PASSES,
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
            msaa_texture: None,
            clear_color: Color::WHITE,
            preparing: false,
            seq_dirty: Vec::new(),
            seq_remove: Vec::new(),
            sequence: Vec::new(),
            last_redraw: None,
            timestamp_poll: false,
            timestamp_mapper,
            timestamp_resolver,
            timestamp_query,
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
                TextureFormat::Rgba16Float => 50,
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

    fn redraw(world: &mut World) {
        // prepare controls

        let mut render = world.single_fetch_mut::<Render>().unwrap();
        render.preparing = true;
        drop(render);

        let mut refreshing = false;
        world.foreach_enter::<Camera>(|_| {
            world.foreach_fetch_mut::<RenderControl>(|mut control| {
                if let Some(prepare) = &mut control.prepare
                    && let Some(info) = prepare(world)
                {
                    refreshing |= info.keep_redrawing;
                };
            });
        });

        world.flush();

        // start redrawing

        let render = &mut *world.single_fetch_mut::<Render>().unwrap();
        render.preparing = false;
        let now = Instant::now();

        // order redraw sequence

        'r: for (dirty, view, ord) in render.seq_dirty.drain(..) {
            for (control, old_view, old_ord) in &mut render.sequence {
                if *control == dirty {
                    *old_view = view;
                    *old_ord = ord;
                    continue 'r;
                }
            }

            // if new
            render.sequence.push((dirty, view, ord));
        }

        (render.sequence).retain(|(control, ..)| !render.seq_remove.contains(control));
        render.seq_remove.clear();

        render.sequence.sort_by(|(.., a), (.., b)| a.cmp(b));

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
        };

        // draw

        for &(control, view, _) in &render.sequence {
            world.enter(view, || {
                let mut control = world.fetch_mut(control).unwrap();
                if let Some(draw) = &mut control.draw {
                    draw(
                        world,
                        &mut rpass,
                        RenderExtra {
                            device: &render.device,
                            queue: &render.queue,
                            early_encoder: &mut early_encoder,
                            surface_config: &render.config,
                            diagnosis: &mut diagnosis,
                        },
                    );
                }
            });
        }

        drop(rpass);

        // GPU timestamp resolve

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

        // active refreshing

        if refreshing {
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.request_redraw();
        }

        // tasks submission

        let commands = [early_encoder.finish(), encoder.finish()];
        render.queue.submit(commands);
        render.queue.present(surface);

        // GPU profiling

        if render.timestamp_poll {
            render.timestamp_mapper.map_async(MapMode::Read, .., |_| {});
            render.device.poll(PollType::wait_indefinitely()).unwrap();
            let period = render.queue.get_timestamp_period() as u64;
            let view = render.timestamp_mapper.get_mapped_range(..).unwrap();
            let (chunks, _) = view.as_chunks::<8>();
            let mut timestamps = [0u64; TIMESTAMP_COUNT as usize];
            for (i, &chunk) in chunks.iter().enumerate() {
                timestamps[i] = u64::from_le_bytes(chunk) * period;
            }
            drop(view);
            render.timestamp_mapper.unmap();

            let mut pairs = indexmap::IndexMap::<String, Duration>::new();
            for ((start, end), name) in diagnosis.slots {
                let duration = pairs.entry(name).or_default();
                *duration += Duration::from_nanos(timestamps[end] - timestamps[start]);
            }

            pairs.sort_unstable_keys();

            let mut output = String::new();

            output += &format!(
                "CPU redraw time: {:.3?}\n",
                Instant::now().duration_since(now)
            );

            if let Some(last) = render.last_redraw {
                output += &format!(
                    "CPU frame time: {:.3?} | {}\n",
                    (now - last),
                    match refreshing {
                        true => "ACTIVE",
                        false => "INACTIVE",
                    },
                )
            };

            output += &format!("GPU timestamp period: {} ns\n", period);

            for (name, duration) in pairs {
                let milis = duration.as_secs_f64() * (1e3f64);
                output += &format!("{name:<30}{milis:>6.3} ms\n");
            }

            world.queue_trigger(world.single::<Render>().unwrap(), output);
        }

        // CPU time tracing

        render.last_redraw = Some(now);
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
        timestamp_writes: Some(RenderPassTimestampWrites {
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
        timestamp_writes: Some(RenderPassTimestampWrites {
            query_set: &render.timestamp_query,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }),
        ..Default::default()
    })
}

impl RenderControl {
    /// Safer functions to request redraw.
    pub fn redraw(world: &World) {
        let render = world.single_fetch::<Render>().unwrap();
        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();

        if !render.preparing {
            lnwindow.window.request_redraw();
        }
    }

    pub fn reorder(order: Option<isize>, world: &World, handle: Handle<Self>) {
        let mut render = world.single_fetch_mut::<Render>().unwrap();

        if let Some(order) = order {
            render.seq_dirty.push((handle, world.here(), order));
            render.seq_remove.retain(|&x| x != handle);
        } else {
            render.seq_remove.push(handle);
        }
    }
}

impl RenderDiagnosis<'_> {
    pub fn assign(&mut self, name: &str) -> (u32, u32) {
        self.assign_string(name.into())
    }

    pub fn assign_string(&mut self, name: String) -> (u32, u32) {
        assert!(
            self.front + 1 < TIMESTAMP_BUFFER_SIZE as usize,
            "too many timestamps"
        );

        let pair = (self.front, self.front + 1);
        self.slots.push((pair, name));
        self.front += 2;
        (pair.0 as u32, pair.1 as u32)
    }

    pub fn write(&mut self, rpass: &mut RenderPass, index: u32) {
        rpass.write_timestamp(&self.query, index);
    }
}

impl Element for Render {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| match event {
            WindowEvent::SurfaceResized(size) => {
                let mut render = world.fetch_mut(this).unwrap();
                render.surface_resize(*size);
            }

            WindowEvent::RedrawRequested => {
                world.queue(|world| {
                    Render::redraw(world);
                });
            }

            _ => (),
        });
    }
}

impl Element for RenderControl {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let render = world.single::<Render>().unwrap();
        world.dependency(this, render);
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        Self::reorder(None, world, this);
    }
}
