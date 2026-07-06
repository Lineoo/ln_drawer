use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use ln_world::{Element, Handle, World};
use palette::{Srgba, WithAlpha};
use swash::scale::image::Content;

use crate::{
    measures::Rectangle,
    render::{
        RenderControl,
        canvas::{Canvas, CanvasDescriptor},
    },
    widgets::{WidgetEnabled, WidgetRectangle},
};

pub struct Text {
    pub text: String,
    pub rect: Rectangle,
    pub metrics: Metrics,
    pub color: Srgba<u8>,
    pub upscale: f32,
    pub order: isize,
    pub visible: bool,
    /// will delay text draw to next render prepare phase
    pub outdated: bool,
}

pub struct TextChanged;

struct TextPipeline {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            text: Default::default(),
            rect: Rectangle::new(0, 0, 200, 24),
            metrics: Metrics::new(24.0, 20.0),
            color: Srgba::new(0, 0, 0, 0),
            upscale: 2.0,
            order: 100,
            visible: true,
            outdated: false,
        }
    }
}

impl Text {
    pub fn init(world: &mut World) {
        let mut font_system = FontSystem::new();
        let database = font_system.db_mut();

        let sans = include_bytes!("../../fonts/SourceHanSansCN-Regular.otf").to_vec();
        let serif = include_bytes!("../../fonts/SourceHanSerifCN-Regular.otf").to_vec();

        database.load_font_data(sans);
        database.load_font_data(serif);

        let swash_cache = SwashCache::new();

        world.insert(TextPipeline {
            font_system,
            swash_cache,
        });
    }

    pub fn bind_render(&mut self, world: &World, this: Handle<Text>) {
        let upscale_width = self.rect.width() as f32 * self.upscale;
        let upscale_height = self.rect.height() as f32 * self.upscale;
        let upscale_width_int = upscale_width.ceil() as u32;
        let upscale_height_int = upscale_height.ceil() as u32;

        let canvas = world.build(CanvasDescriptor {
            data: None,
            data_width: upscale_width_int,
            data_height: upscale_height_int,
            rect: self.rect,
            order: self.order,
            visible: self.visible,
        });

        let upscale_metrics = self.metrics.scale(self.upscale);
        let manager = &mut *world.single_fetch_mut::<TextPipeline>().unwrap();
        let mut buffer = Buffer::new(&mut manager.font_system, upscale_metrics);

        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();

                if !this.outdated || !this.visible {
                    return None;
                }
                this.outdated = false;

                let mut canvas = world.fetch_mut(canvas).unwrap();

                canvas.clear_transparent();

                let upscale_metrics = this.metrics.scale(this.upscale);
                let manager = &mut *world.single_fetch_mut::<TextPipeline>().unwrap();
                let mut buffer_font = buffer.borrow_with(&mut manager.font_system);

                let attrs = Attrs::new().family(Family::Name("Source Han Sans CN"));
                buffer_font.set_metrics(upscale_metrics);
                buffer_font.set_size(Some(upscale_width), Some(upscale_height));
                buffer_font.set_text(&this.text, &attrs, Shaping::Basic);

                draw_buffer(&buffer, &this, &mut canvas, manager);

                canvas.upload_full();

                None
            })),
            draw: None,
        });

        RenderControl::reorder(Some(0), world, control);

        world.observer(this, move |&TextChanged, world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.outdated = true;
        });

        world.observer(this, move |&WidgetEnabled(enabled), world| {
            let mut canvas = world.fetch_mut(canvas).unwrap();
            canvas.visible = enabled;
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut canvas = world.fetch_mut(canvas).unwrap();
            canvas.rect = rect;
        });

        self.outdated = true;
    }
}

fn draw_buffer(buffer: &Buffer, this: &Text, canvas: &mut Canvas, manager: &mut TextPipeline) {
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical_glyph = glyph.physical((0., 0.), 1.0);

            let Some(image) = manager
                .swash_cache
                .get_image(&mut manager.font_system, physical_glyph.cache_key)
            else {
                continue;
            };

            let x = image.placement.left;
            let y = -image.placement.top;

            match image.content {
                Content::Mask => {
                    let mut i = 0;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            canvas.draw_over(
                                physical_glyph.x + x + off_x,
                                run.line_y as i32 + physical_glyph.y + y + off_y,
                                this.color.with_alpha(image.data[i]).into_format(),
                            );
                            i += 1;
                        }
                    }
                }
                Content::Color => {
                    let mut i = 0;
                    for off_y in 0..image.placement.height as i32 {
                        for off_x in 0..image.placement.width as i32 {
                            canvas.draw_over(
                                x + off_x,
                                y + off_y,
                                Srgba::<u8>::new(
                                    image.data[i],
                                    image.data[i + 1],
                                    image.data[i + 2],
                                    image.data[i + 3],
                                )
                                .into_format(),
                            );
                            i += 4;
                        }
                    }
                }
                Content::SubpixelMask => {
                    log::warn!("TODO: SubpixelMask");
                }
            }
        }
    }
}

impl Element for Text {}
impl Element for TextPipeline {}
