use std::sync::Arc;

use cosmic_text::*;
use parking_lot::Mutex;
use wgpu::{Device, Queue, RenderPass, SurfaceConfiguration};

use crate::interface::{InterfaceViewport, Painter, painter::PainterPipeline};

pub struct TextManager {
    inner_pipeline: PainterPipeline,

    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,
}
impl TextManager {
    pub fn init(
        device: &Device,
        surface: &SurfaceConfiguration,
        viewport: &InterfaceViewport,
    ) -> Self {
        let inner_pipeline = PainterPipeline::init(device, surface, viewport);
        let font_system = Arc::new(Mutex::new(FontSystem::new()));
        let swash_cache = Arc::new(Mutex::new(SwashCache::new()));
        TextManager {
            inner_pipeline,
            font_system,
            swash_cache,
        }
    }

    #[must_use = "The text will be destroyed when being drop."]
    pub fn create(&mut self, rect: [i32; 4], text: &str, device: &Device, queue: &Queue) -> Text {
        let mut font_system = self.font_system.lock();
        let mut swash_cache = self.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut font_system);

        let width = (rect[0] - rect[2]).unsigned_abs();
        let height = (rect[1] - rect[3]).unsigned_abs();

        let attrs = Attrs::new();
        buffer.set_size(Some(width as f32), Some(height as f32));
        buffer.set_text(text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(true);

        let mut data = vec![0; (width * height * 4) as usize];

        buffer.draw(
            &mut swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * width as i32) * 4) as usize;
                let rgba = color.as_rgba();
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        let inner = self.inner_pipeline.create(rect, data, device, queue);

        Text {
            inner,
            font_system: self.font_system.clone(),
            swash_cache: self.swash_cache.clone(),
        }
    }

    pub fn clean(&mut self) {
        self.inner_pipeline.clean();
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        self.inner_pipeline.render(rpass);
    }
}

pub struct Text {
    inner: Painter,
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,
}
impl Text {
    pub fn set_text(&mut self, text: &str, color: [u8; 4]) {
        let mut font_system = self.font_system.lock();
        let mut swash_cache = self.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer = buffer.borrow_with(&mut font_system);

        let rect = self.inner.get_rect();
        let width = (rect[0] - rect[2]).unsigned_abs();
        let height = (rect[1] - rect[3]).unsigned_abs();

        let attrs = Attrs::new();
        buffer.set_size(Some(width as f32), Some(height as f32));
        buffer.set_text(text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(true);

        let mut writer = self.inner.start_writer();

        buffer.draw(
            &mut swash_cache,
            Color::rgba(color[0], color[1], color[2], color[3]),
            |x, y, _, _, color| {
                let rgba = color.as_rgba();
                writer.set_pixel(x, y, rgba);
            },
        );
    }

    pub fn get_border(&self) -> [i32; 4] {
        self.inner.get_rect()
    }

    pub fn get_position(&self) -> [i32; 2] {
        self.inner.get_position()
    }

    pub fn set_position(&mut self, position: [i32; 2]) {
        self.inner.set_position(position);
    }
}
