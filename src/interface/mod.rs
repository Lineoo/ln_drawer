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

    camera: [i32; 2],
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

        // Camera
        let camera = [0, 0];

        // Render Components
        let wireframe = wireframe::WireframePipeline::init(&device, &surface_config);
        let painter = painter::PainterPipeline::init(&device, &surface_config);

        Interface {
            surface,
            device,
            queue,
            width,
            height,
            wireframe,
            painter,
            camera,
        }
    }

    /// Suggested to call before [`Interface::redraw()`]. This will following jobs:
    /// - Remove unattached components
    /// - Update components' data through channel
    pub fn restructure(&mut self) {
        self.wireframe.update_rect(&self.viewport(), &self.queue);
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

    #[must_use = "The wireframe will be destroyed when being drop."]
    pub fn create_wireframe(&mut self, rect: [i32; 4], color: [f32; 4]) -> Wireframe {
        self.wireframe
            .create(rect, color, &self.device, &self.queue, &self.viewport())
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn create_painter(&mut self, rect: [i32; 4], width: u32, height: u32) -> Painter {
        self.painter.create(
            rect,
            width,
            height,
            &self.device,
            &self.queue,
            &self.viewport(),
        )
    }

    pub fn get_camera(&self) -> [i32; 2] {
        self.camera
    }

    pub fn set_camera(&mut self, position: [i32; 2]) {
        self.camera = position;
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    fn viewport(&self) -> InterfaceViewport {
        InterfaceViewport {
            width: self.width,
            height: self.height,
            camera: self.camera,
        }
    }
}

struct InterfaceViewport {
    width: u32,
    height: u32,

    camera: [i32; 2],
}
impl InterfaceViewport {
    fn world_to_screen(&self, point: [i32; 2]) -> [f32; 2] {
        let x = (point[0] - self.camera[0]) as f32 / self.width as f32 * 2.0;
        let y = (point[1] - self.camera[1]) as f32 / self.height as f32 * 2.0;
        [x, y]
    }
}
