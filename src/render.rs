pub mod canvas;
pub mod rounded;
pub mod text;
pub mod vertex;
pub mod viewport;
pub mod wireframe;

use wgpu::{
    Adapter, Color, CommandEncoder, CommandEncoderDescriptor, CompositeAlphaMode, Device,
    DeviceDescriptor, Features, Instance, Limits, LoadOp, MemoryHints, Operations, PowerPreference,
    Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions,
    StoreOp, Surface, SurfaceConfiguration, SurfaceTarget, TextureFormat, TextureUsages,
    TextureViewDescriptor, Trace,
};
use winit::dpi::PhysicalSize;

use crate::world::{Element, World};

pub struct Render {
    surface: Surface<'static>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    pub active: Option<RenderActive>,
}

pub struct RenderActive {
    encoder: CommandEncoder,
    pub rpass: RenderPass<'static>,
}

#[derive(Debug)]
pub struct RenderControl {
    pub visible: bool,
    pub order: isize,
}

impl Element for Render {}
impl Element for RenderControl {}

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
        }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        let caps = self.surface.get_capabilities(&self.adapter);
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: {
                let caps = &caps.formats;
                if caps.contains(&TextureFormat::Bgra8UnormSrgb) {
                    TextureFormat::Bgra8UnormSrgb
                } else {
                    *caps.first().unwrap()
                }
            },
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode: *caps.present_modes.first().unwrap(),
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

        self.surface.configure(&self.device, &config);
    }

    pub fn redraw(&mut self, world: &World) {
        let mut buf = Vec::with_capacity(world.len::<RenderControl>());
        world.foreach_fetch::<RenderControl>(|control, fetched| {
            if fetched.visible {
                buf.push((control, fetched.order));
            }
        });

        buf.sort_by(|(_, a), (_, b)| a.cmp(b));

        for (control, _) in &buf {
            world.trigger(*control, RedrawPrepare);
        }

        let texture = self.surface.get_current_texture().unwrap();
        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
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
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            })
            .forget_lifetime();

        self.active.replace(RenderActive { encoder, rpass });

        for (control, _) in &buf {
            world.trigger(*control, Redraw);
        }

        world.queue(move |world| {
            let mut render = world.single_fetch_mut::<Render>().unwrap();
            let active = render.active.take().unwrap();

            drop(active.rpass);

            render.queue.submit([active.encoder.finish()]);

            texture.present();
        });
    }
}
