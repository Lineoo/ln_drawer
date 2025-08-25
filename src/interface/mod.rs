use wgpu::*;

mod painter;
mod text;
mod viewport;
mod wireframe;

pub use painter::Painter;
pub use text::Text;
pub use viewport::InterfaceViewport;
pub use wireframe::Wireframe;

/// Main render part
pub struct Interface {
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    device: Device,
    queue: Queue,

    wireframe: wireframe::WireframePipeline,
    painter: painter::PainterPipeline,
    text: text::TextManager,

    viewport: InterfaceViewport,
}
impl Interface {
    pub async fn new(window: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Interface {
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

        // Surface Configuration
        let width = width.max(1);
        let height = height.max(1);

        let surface_config = surface.get_default_config(&adapter, width, height).unwrap();

        // Camera
        let camera = [0, 0];
        let zoom = 1.0;
        let viewport = InterfaceViewport::new(&device, width, height, camera, zoom);

        // Render Components
        let wireframe = wireframe::WireframePipeline::init(&device, &surface_config, &viewport);
        let painter = painter::PainterPipeline::init(&device, &surface_config, &viewport);
        let text = text::TextManager::init(&device, &surface_config, &viewport);

        Interface {
            surface,
            surface_config,
            device,
            queue,
            wireframe,
            painter,
            text,
            viewport,
        }
    }

    /// Suggested to call before [`Interface::redraw()`]. This will following jobs:
    /// - Remove unattached components
    pub fn restructure(&mut self) {
        self.painter.clean();
        self.wireframe.clean();
        self.wireframe.update_visibility();
        self.text.clean();
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
        self.text.render(&mut rpass);

        drop(rpass);

        self.queue.submit([encoder.finish()]);

        texture.present();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        self.viewport.resize(width, height, &self.queue);

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
        let width = (rect[0] - rect[2]).unsigned_abs();
        let height = (rect[1] - rect[3]).unsigned_abs();
        let empty = vec![0; (width * height * 4) as usize];
        self.painter.create(rect, empty, &self.device, &self.queue)
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn create_painter_with(&mut self, rect: [i32; 4], data: Vec<u8>) -> Painter {
        self.painter.create(rect, data, &self.device, &self.queue)
    }

    #[must_use = "The text will be destroyed when being drop."]
    pub fn create_text(&mut self, rect: [i32; 4], text: &str) -> Text {
        self.text.create(rect, text, &self.device, &self.queue)
    }

    // Viewport Shortcut //

    pub fn get_camera(&self) -> [i32; 2] {
        self.viewport.get_camera()
    }

    pub fn set_camera(&mut self, position: [i32; 2]) {
        self.viewport.set_camera(position, &self.queue);
    }

    pub fn get_zoom(&self) -> f32 {
        self.viewport.get_zoom()
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.viewport.set_zoom(zoom, &self.queue);
    }

    pub fn world_to_screen(&self, point: [i32; 2]) -> [f64; 2] {
        self.viewport.world_to_screen(point)
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> [i32; 2] {
        self.viewport.screen_to_world(point)
    }

    /// This ignore the camera position, useful for relative point like mouse dragging
    pub fn screen_to_world_relative(&self, point: [f64; 2]) -> [i32; 2] {
        self.viewport.screen_to_world_relative(point)
    }
}
