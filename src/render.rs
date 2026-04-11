pub mod camera;
pub mod canvas;
pub mod rectangle;
pub mod rounded;
pub mod text;
pub mod vertex;
pub mod wireframe;

use std::time::Instant;

use wgpu::{
    Adapter, Color, CommandEncoderDescriptor, CompositeAlphaMode, Device, DeviceDescriptor,
    ExperimentalFeatures, Extent3d, Features, Instance, Limits, LoadOp, MemoryHints,
    MultisampleState, Operations, PowerPreference, PresentMode, Queue, RenderPass,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface,
    SurfaceConfiguration, Texture, TextureDescriptor, TextureDimension, TextureUsages,
    TextureViewDescriptor, Trace,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{
    lnwin::Lnwindow,
    render::camera::Camera,
    world::{Element, Handle, World},
};

pub const MSAA_SAMPLE_COUNT: u32 = 4;
pub const MSAA_STATE: MultisampleState = MultisampleState {
    count: MSAA_SAMPLE_COUNT,
    mask: !0,
    alpha_to_coverage_enabled: false,
};

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
    msaa_texture: Texture,

    // render pass
    pub clear_color: Color,

    // render control
    preparing: bool,
    seq_dirty: Vec<(Handle<RenderControl>, Handle, isize)>,
    seq_remove: Vec<Handle<RenderControl>>,
    sequence: Vec<(Handle<RenderControl>, Handle, isize)>,

    // time tracing
    last_redraw: Option<Instant>,
}

type RenderPrepareCommand = Box<dyn FnMut(&World) -> Option<RenderInformation>>;
type RenderDrawCommand = Box<dyn FnMut(&World, &mut RenderPass<'static>)>;

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

impl Render {
    pub async fn new(lnwindow: &Lnwindow) -> Render {
        let instance = Instance::default();

        let surface = instance.create_surface(lnwindow.window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        log::debug!("wgpu adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
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

        let msaa_texture = device.create_texture(&Render::msaa_texel(size, &config));

        Render {
            surface,
            config,
            instance,
            adapter,
            device,
            queue,
            msaa_texture,
            clear_color: Color::WHITE,
            preparing: false,
            seq_dirty: Vec::new(),
            seq_remove: Vec::new(),
            sequence: Vec::new(),
            last_redraw: None,
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

        let desc = Render::msaa_texel(size, &self.config);
        self.msaa_texture = self.device.create_texture(&desc);
    }

    pub fn surface_resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);

        let desc = Render::msaa_texel(size, &self.config);
        self.msaa_texture = self.device.create_texture(&desc);
    }

    fn msaa_texel(size: PhysicalSize<u32>, config: &SurfaceConfiguration) -> TextureDescriptor<'_> {
        TextureDescriptor {
            label: Some("render_msaa"),
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: MSAA_SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format: config.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TRANSIENT,
            view_formats: &[],
        }
    }

    fn configuration(
        surface: &Surface,
        adapter: &Adapter,
        size: PhysicalSize<u32>,
    ) -> SurfaceConfiguration {
        let caps = surface.get_capabilities(&adapter);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: *caps.formats.first().unwrap(),
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
            view_formats: vec![],
        };

        log::trace!("resize in {}, {}", config.width, config.height);
        log::trace!("present mode {:?} is selected", config.present_mode);
        log::trace!("alpha mode {:?} is selected", config.alpha_mode);

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

        let texture = render.surface.get_current_texture().unwrap();
        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());
        let msaa_view = render
            .msaa_texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = render
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("main_encoder"),
            });

        let mut rpass = encoder
            .begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&view),
                    ops: Operations {
                        load: LoadOp::Clear(render.clear_color),
                        store: StoreOp::Discard,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            })
            .forget_lifetime();

        // draw and submit

        for &(control, view, _) in &render.sequence {
            world.enter(view, || {
                let mut control = world.fetch_mut(control).unwrap();
                if let Some(render) = &mut control.draw {
                    render(world, &mut rpass);
                }
            });
        }

        drop(rpass);
        render.queue.submit([encoder.finish()]);
        texture.present();

        // active refreshing

        if refreshing {
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.request_redraw();
        }

        // time tracing

        if let Some(last) = render.last_redraw {
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.set_title(&format!(
                "frame time: {:.4} | {}",
                (now - last).as_secs_f32(),
                match refreshing {
                    true => "ACTIVE",
                    false => "INACTIVE",
                },
            ));
        }

        render.last_redraw = Some(now);
    }
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
