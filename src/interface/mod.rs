use std::sync::Arc;

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

mod painter;
mod wireframe;

pub use painter::Painter;
pub use wireframe::Wireframe;

/// Main render part
pub struct Interface {
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    device: Device,
    queue: Queue,

    wireframe: wireframe::WireframePipeline,
    painter: painter::PainterPipeline,

    viewport: InterfaceViewport,
}
impl Interface {
    pub async fn new(window: Arc<winit::window::Window>) -> Interface {
        let instance = Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

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

        // Surface Configuration
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_config = surface.get_default_config(&adapter, width, height).unwrap();

        // Camera
        let camera = [0, 0];
        let viewport_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("viewport_buffer"),
            contents: bytemuck::bytes_of(&[width as i32, height as i32, camera[0], camera[1]]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let viewport = InterfaceViewport {
            width,
            height,
            camera,
            buffer: viewport_buffer,
        };

        // Render Components
        let wireframe = wireframe::WireframePipeline::init(&device, &surface_config, &viewport);
        let painter = painter::PainterPipeline::init(&device, &surface_config, &viewport);

        Interface {
            surface,
            surface_config,
            device,
            queue,
            wireframe,
            painter,
            viewport,
        }
    }

    /// Suggested to call before [`Interface::redraw()`]. This will following jobs:
    /// - Remove unattached components
    pub fn restructure(&mut self) {
        self.painter.clean();
        self.wireframe.clean();
    }

    pub fn redraw(&self) {
        let texture = self.surface.get_current_texture().unwrap();

        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });

        self.painter.render(&mut rpass);
        self.wireframe.render(&mut rpass);

        drop(rpass);

        self.queue.submit([encoder.finish()]);

        texture.present();
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let width = size.width.max(1);
        let height = size.height.max(1);

        self.viewport.width = width;
        self.viewport.height = height;
        self.queue.write_buffer(
            &self.viewport.buffer,
            0,
            bytemuck::bytes_of(&[width as i32, height as i32]),
        );

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    #[must_use = "The wireframe will be destroyed when being drop."]
    pub fn create_wireframe(&mut self, rect: [i32; 4], color: [f32; 4]) -> Wireframe {
        self.wireframe
            .create(rect, color, &self.device, &self.queue)
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn create_painter(&mut self, rect: [i32; 4]) -> Painter {
        self.painter.create(rect, &self.device, &self.queue)
    }

    pub fn get_camera(&self) -> [i32; 2] {
        self.viewport.camera
    }

    pub fn set_camera(&mut self, position: [i32; 2]) {
        self.viewport.camera = position;
        self.queue.write_buffer(
            &self.viewport.buffer,
            size_of::<[u32; 2]>() as BufferAddress,
            bytemuck::bytes_of(&position),
        );
    }
}

struct InterfaceViewport {
    width: u32,
    height: u32,

    camera: [i32; 2],

    buffer: Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InterfaceViewportBind {
    width: i32,
    height: i32,
    camera: [i32; 2],
}
