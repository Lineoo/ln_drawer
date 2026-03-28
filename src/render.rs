pub mod canvas;
pub mod rounded;
pub mod text;
pub mod vertex;
pub mod viewport;
pub mod wireframe;
pub mod rectangle;

use std::time::{Duration, Instant};

use wgpu::{
    Adapter, Color, CommandEncoder, CommandEncoderDescriptor, CompositeAlphaMode, Device,
    DeviceDescriptor, ExperimentalFeatures, Features, Instance, Limits, LoadOp, MemoryHints,
    Operations, PowerPreference, PresentMode, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration,
    TextureFormat, TextureUsages, TextureViewDescriptor, Trace,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{
    lnwin::Lnwindow,
    world::{Element, Handle, World},
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

    // render pass
    pub clear_color: Color,

    // render control
    sequence: Vec<Handle<RenderControl>>,
    refreshing: bool,

    // time tracing
    last_redraw: Option<Instant>,
    last_control: Option<Instant>,
    last_lossy: Option<Instant>,
}

#[deprecated]
struct RenderPortal {
    active: Option<RenderActive>,
    redrawing: bool,
}

#[deprecated]
struct RenderActive {
    encoder: CommandEncoder,
    rpass: RenderPass<'static>,
}

type RenderPrepareCommand = Box<dyn FnMut(&World) -> Option<RenderInformation>>;
type RenderDrawCommand = Box<dyn FnMut(&World, &mut RenderPass<'static>)>;

pub struct RenderControl {
    pub visible: bool,
    pub order: isize,
    pub refreshing: bool,

    /// prepare to render and give related information
    pub prepare: Option<RenderPrepareCommand>,

    /// draw with given render pass
    pub draw: Option<RenderDrawCommand>,
}

pub struct RenderInformation {
    pub render_order: isize,
    pub keep_redrawing: bool,
}

#[deprecated]
pub struct LossyPrepare;

#[deprecated]
pub struct RedrawPrepare;
#[deprecated]
pub struct Redraw;

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

        Render {
            surface,
            config,
            instance,
            adapter,
            device,
            queue,
            clear_color: Color::WHITE,
            sequence: Vec::new(),
            refreshing: false,
            last_redraw: None,
            last_control: None,
            last_lossy: None,
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
}

impl Element for Render {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let portal = world.insert(RenderPortal {
            active: None,
            redrawing: false,
        });

        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| match event {
            WindowEvent::SurfaceResized(size) => {
                let mut render = world.fetch_mut(this).unwrap();
                render.config.width = size.width.max(1);
                render.config.height = size.height.max(1);
                render.surface.configure(&render.device, &render.config);
            }

            WindowEvent::RedrawRequested => {
                let mut render = world.fetch_mut(this).unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let now = Instant::now();

                // start redrawing

                let mut rportal = world.fetch_mut(portal).unwrap();
                rportal.redrawing = true;
                drop(rportal);

                // render control

                if render
                    .last_control
                    .is_none_or(|last| now - last > Duration::from_millis(10))
                {
                    let mut refreshing = false;

                    let mut buf = Vec::with_capacity(world.size_hint::<RenderControl>());
                    world.foreach_fetch_mut::<RenderControl>(|mut control| {
                        if let Some(prepare) = &mut control.prepare {
                            let info = prepare(world);
                            if let Some(info) = info {
                                buf.push((control.handle(), info.render_order));
                                refreshing |= info.keep_redrawing;
                            }

                            return;
                        };

                        if control.visible {
                            buf.push((control.handle(), control.order));
                        }

                        if control.refreshing {
                            refreshing = true;
                        }
                    });

                    buf.sort_by(|(_, a), (_, b)| a.cmp(b));

                    render.sequence.clear();
                    render.sequence.reserve(buf.len());
                    for (control, _) in buf {
                        render.sequence.push(control);
                    }

                    render.refreshing = refreshing;
                    render.last_control = Some(now);
                }

                // lossy redraw prepare

                if render
                    .last_lossy
                    .is_none_or(|last| now - last > Duration::from_millis(100))
                {
                    for control in &render.sequence {
                        world.trigger(*control, &LossyPrepare);
                    }

                    render.last_lossy = Some(now);
                }

                // redraw prepare

                for control in &render.sequence {
                    world.trigger(*control, &RedrawPrepare);
                }

                // setup render pass

                let texture = render.surface.get_current_texture().unwrap();
                let view = texture
                    .texture
                    .create_view(&TextureViewDescriptor::default());

                let mut encoder = render
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("main_encoder"),
                    });

                let mut rpass = encoder
                    .begin_render_pass(&RenderPassDescriptor {
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(render.clear_color),
                                store: StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        ..Default::default()
                    })
                    .forget_lifetime();

                // call everyone to draw

                // FIXME why it's here
                render.sequence.retain(|x| world.available_mut(*x).is_ok());

                for control in &render.sequence {
                    let mut control = world.fetch_mut(*control).unwrap();
                    if let Some(render) = &mut control.draw {
                        render(world, &mut rpass);
                    }
                }

                let mut rportal = world.fetch_mut(portal).unwrap();
                rportal.active.replace(RenderActive { encoder, rpass });
                drop(rportal);

                for control in &render.sequence {
                    world.trigger(*control, &Redraw);
                }

                let mut rportal = world.fetch_mut(portal).unwrap();
                let active = rportal.active.take().unwrap();

                // submit to GPU

                drop(active.rpass);
                render.queue.submit([active.encoder.finish()]);
                texture.present();

                // active refreshing

                if render.refreshing {
                    lnwindow.window.request_redraw();
                }

                // record time

                if let Some(last) = render.last_redraw {
                    lnwindow.window.set_title(&format!(
                        "frame time: {:.4} | {}",
                        (now - last).as_secs_f32(),
                        match render.refreshing {
                            true => "ACTIVE",
                            false => "INACTIVE",
                        },
                    ));
                }

                // stop redrawing

                render.last_redraw = Some(now);
                rportal.redrawing = false;
            }

            _ => (),
        });
    }
}

impl Element for RenderPortal {}

impl Element for RenderControl {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(this, world.single::<RenderPortal>().unwrap());
        world.dependency(this, world.single::<Lnwindow>().unwrap());
        determine_redraw(self, world);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        determine_redraw(self, world);
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        determine_redraw(self, world);
    }
}

fn determine_redraw(control: &RenderControl, world: &World) {
    let rportal = world.single_fetch::<RenderPortal>().unwrap();
    if rportal.redrawing {
        // warn if it's in Redraw phase, ignore if it's in Prepare phase
        if rportal.active.is_some() {
            log::warn!("loop redraw detected");
        }

        return;
    }

    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
    lnwindow.window.request_redraw();
}
