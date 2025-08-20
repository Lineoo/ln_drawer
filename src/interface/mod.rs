use std::sync::Arc;

use wgpu::*;

mod painter;
mod wireframe;

pub use painter::Painter;
pub use wireframe::Wireframe;

/// Main render part
pub struct Interface {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,

    width: u32,
    height: u32,

    wireframe: wireframe::WireframePipeline,
    painter: painter::PainterPipeline,
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

        surface.configure(&device, &surface_config);

        // Render Components
        let mut wireframe = wireframe::WireframePipeline::init(&device, &surface_config);
        wireframe.create([0.0, 0.0, 0.2, 0.5], [1.0, 0.0, 0.0, 1.0], &device);
        let painter = painter::PainterPipeline::init(&device, &surface_config);

        Interface {
            surface,
            device,
            queue,
            width,
            height,
            wireframe,
            painter,
        }
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

    pub fn create_wireframe(&mut self, rect: [f32; 4], color: [f32; 4]) -> Wireframe {
        self.wireframe.create(rect, color, &self.device)
    }

    pub fn create_wireframe_instance(&mut self, rect: [f32; 4], color: [f32; 4]) -> Arc<Wireframe> {
        self.wireframe.create_instance(rect, color, &self.device)
    }

    pub fn create_painter(&mut self, rect: [f32; 4], width: u32, height: u32) -> Arc<Painter> {
        self.painter.create(rect, width, height, &self.device)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}
