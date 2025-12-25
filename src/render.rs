pub mod canvas;
pub mod rounded;
pub mod text;
pub mod vertex;
pub mod viewport;
pub mod wireframe;

use std::time::Instant;

use wgpu::{
    Adapter, Color, CommandEncoder, CommandEncoderDescriptor, CompositeAlphaMode, Device,
    DeviceDescriptor, Features, Instance, Limits, LoadOp, MemoryHints, Operations, PowerPreference,
    PresentMode, Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, SurfaceTarget, TextureUsages,
    TextureViewDescriptor, Trace,
};
use winit::event::WindowEvent;

use crate::{
    lnwin::Lnwindow,
    world::{Element, Handle, World},
};

pub struct Render {
    surface: Surface<'static>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    pub active: Option<RenderActive>,
    pub clear_color: Color,
    pub last_redraw: Instant,
}

pub struct RenderActive {
    encoder: CommandEncoder,
    pub rpass: RenderPass<'static>,
}

#[derive(Debug)]
pub struct RenderControl {
    pub visible: bool,
    pub order: isize,
    pub refreshing: bool,
}

pub struct RedrawPrepare;
pub struct Redraw;

impl Render {
    pub async fn new(window: impl Into<SurfaceTarget<'static>>) -> Render {
        let instance = Instance::default();

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::defaults(),
                memory_hints: MemoryHints::MemoryUsage,
                trace: Trace::Off,
            })
            .await
            .unwrap();

        Render {
            surface,
            adapter,
            device,
            queue,
            active: None,
            clear_color: Color::BLACK,
            last_redraw: Instant::now(),
        }
    }
}

impl Element for Render {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world, _| match event {
            WindowEvent::Resized(size) => {
                let render = world.fetch(this).unwrap();
                let caps = render.surface.get_capabilities(&render.adapter);
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

                log::debug!("present mode {:?} is selected", config.present_mode);
                log::debug!("alpha mode {:?} is selected", config.alpha_mode);

                render.surface.configure(&render.device, &config);
            }

            WindowEvent::RedrawRequested => {
                // sorting phrase

                let mut refreshing = false;

                let mut buf = Vec::with_capacity(world.len::<RenderControl>());
                world.foreach_fetch::<RenderControl>(|control, fetched| {
                    if fetched.visible {
                        buf.push((control, fetched.order));
                    }

                    if fetched.refreshing {
                        refreshing = true;
                    }
                });

                buf.sort_by(|(_, a), (_, b)| a.cmp(b));

                // redraw prepare

                for (control, _) in &buf {
                    world.trigger(*control, RedrawPrepare);
                }

                // setup render pass

                let mut render = world.fetch_mut(this).unwrap();

                let texture = render.surface.get_current_texture().unwrap();
                let view = texture
                    .texture
                    .create_view(&TextureViewDescriptor::default());

                let mut encoder = render
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("main_encoder"),
                    });

                let rpass = encoder
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

                render.active.replace(RenderActive { encoder, rpass });

                drop(render);

                // call everyone to draw

                for (control, _) in &buf {
                    world.trigger(*control, Redraw);
                }

                // submit to GPU

                let mut render = world.single_fetch_mut::<Render>().unwrap();
                let active = render.active.take().unwrap();

                drop(active.rpass);

                render.queue.submit([active.encoder.finish()]);

                texture.present();

                // active refreshing

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                if refreshing {
                    lnwindow.request_redraw();
                }

                // record time

                let now = Instant::now();
                lnwindow.window.set_title(&format!(
                    "frame time: {:.4} | {}",
                    (now - render.last_redraw).as_secs_f32(),
                    match refreshing {
                        true => "ACTIVE",
                        false => "INACTIVE",
                    }
                ));

                render.last_redraw = now;
            }

            _ => (),
        });
    }
}

impl Element for RenderControl {}
