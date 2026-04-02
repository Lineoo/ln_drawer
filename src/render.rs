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
    ExperimentalFeatures, Features, Instance, Limits, LoadOp, MemoryHints, MultisampleState,
    Operations, PowerPreference, PresentMode, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration,
    TextureUsages, TextureViewDescriptor, Trace,
};
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{
    lnwin::Lnwindow,
    render::camera::CameraVisits,
    world::{Element, Handle, World},
};

pub const MSAA_STATE: MultisampleState = MultisampleState {
    count: 4,
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

    // render pass
    pub clear_color: Color,

    // render control
    redrawing: bool,

    // time tracing
    last_redraw: Option<Instant>,
}

type RenderPrepareCommand = Box<dyn FnMut(&World) -> Option<RenderInformation>>;
type RenderDrawCommand = Box<dyn FnMut(&World, &mut RenderPass<'static>)>;

pub struct RenderControl {
    /// prepare to render and give related information
    pub prepare: Option<RenderPrepareCommand>,

    /// draw with given render pass
    pub draw: Option<RenderDrawCommand>,
}

pub struct RenderInformation {
    pub render_order: isize,
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

        Render {
            surface,
            config,
            instance,
            adapter,
            device,
            queue,
            clear_color: Color::WHITE,
            redrawing: false,
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

    fn redraw(world: &World) {
        // start rendering

        let mut render = world.single_fetch_mut::<Render>().unwrap();
        render.redrawing = true;
        drop(render);

        let now = Instant::now();

        // prepare controls

        let mut refreshing = false;
        let mut sorting_phase = Vec::with_capacity(world.size_hint::<RenderControl>());

        let visits = world.single_fetch::<CameraVisits>().unwrap();
        for &view in &visits.views {
            world.enter(view, || {
                world.foreach_fetch_mut::<RenderControl>(|mut control| {
                    if let Some(prepare) = &mut control.prepare {
                        let info = prepare(world);
                        if let Some(info) = info {
                            sorting_phase.push((control.handle(), view, info.render_order));
                            refreshing |= info.keep_redrawing;
                        }
                    };
                });
            });
        }

        sorting_phase.sort_by(|(.., a), (.., b)| a.cmp(b));
        let mut draw_sequence = Vec::with_capacity(sorting_phase.len());
        for (control, view, _) in sorting_phase {
            draw_sequence.push((control, view));
        }

        // setup render pass

        let render = world.single_fetch::<Render>().unwrap();
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

        // draw and submit

        drop(render);
        for &(control, view) in &draw_sequence {
            world.enter(view, || {
                // FIXME why it's here
                if world.validate(control).is_err() {
                    return;
                }

                let mut control = world.fetch_mut(control).unwrap();
                if let Some(render) = &mut control.draw {
                    render(world, &mut rpass);
                }
            });
        }

        drop(rpass);
        let mut render = world.single_fetch_mut::<Render>().unwrap();
        render.queue.submit([encoder.finish()]);
        texture.present();

        // active refreshing

        if refreshing {
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.request_redraw();
        }

        // record time

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

        // stop redrawing

        render.last_redraw = Some(now);
        render.redrawing = false;
    }
}

impl Element for Render {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| match event {
            WindowEvent::SurfaceResized(size) => {
                let mut render = world.fetch_mut(this).unwrap();
                render.config.width = size.width.max(1);
                render.config.height = size.height.max(1);
                render.surface.configure(&render.device, &render.config);
            }

            WindowEvent::RedrawRequested => {
                Render::redraw(world);
            }

            _ => (),
        });
    }
}

impl Element for RenderControl {}
