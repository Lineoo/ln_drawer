use std::sync::Arc;

use cosmic_text::*;
use parking_lot::Mutex;

use crate::{
    elements::{Element, OrderElement},
    interface::{Interface, Painter},
    measures::Rectangle,
};

pub struct TextManager {
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,
}
impl Default for TextManager {
    fn default() -> Self {
        let font_system = Arc::new(Mutex::new(FontSystem::new()));
        let swash_cache = Arc::new(Mutex::new(SwashCache::new()));
        TextManager {
            font_system,
            swash_cache,
        }
    }
}
impl Element for TextManager {}

pub struct Text {
    inner: Painter,
    text: String,
    buffer: Buffer,
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,
}
impl Text {
    pub fn new(
        rect: Rectangle,
        text: String,
        manager: &mut TextManager,
        interface: &mut Interface,
    ) -> Text {
        let mut font_system = manager.font_system.lock();
        let mut swash_cache = manager.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut font_system);

        let attrs = Attrs::new();
        buffer_borrow.set_size(Some(rect.width() as f32), Some(rect.height() as f32));
        buffer_borrow.set_text(&text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);

        let mut data = vec![0; (rect.width() * rect.height() * 4) as usize];

        buffer_borrow.draw(
            &mut swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * rect.height() as i32) * 4) as usize;
                let rgba = color.as_rgba();
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        Text {
            inner: interface.create_painter_with(rect.into_array(), data),
            text,
            buffer,
            font_system: manager.font_system.clone(),
            swash_cache: manager.swash_cache.clone(),
        }
    }

    pub fn set_text(&mut self, text: &str, color: [u8; 4]) {
        let mut font_system = self.font_system.lock();
        let mut swash_cache = self.swash_cache.lock();

        text.clone_into(&mut self.text);
        self.buffer
            .set_text(&mut font_system, text, &Attrs::new(), Shaping::Advanced);

        let mut writer = self.inner.open_writer();
        self.buffer.draw(
            &mut font_system,
            &mut swash_cache,
            Color::rgba(color[0], color[1], color[2], color[3]),
            |x, y, _, _, color| {
                let rgba = color.as_rgba();
                writer.set_pixel(x, y, rgba);
            },
        );
    }
}
impl Element for Text {}
impl OrderElement for Text {
    fn get_order(&self) -> isize {
        self.inner.get_z_order()
    }

    fn set_order(&mut self, order: isize) {
        self.inner.set_z_order(order);
    }
}
