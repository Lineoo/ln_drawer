use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use ln_world::{Element, Handle, World};
use palette::{Srgba, WithAlpha};
use swash::scale::image::Content;

use crate::{
    measures::Rectangle,
    render::{RenderControl, RenderInformation},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        renderer::canvas::{Canvas, CanvasDescriptor},
    },
};

pub struct Text {
    pub text: String,
    pub rect: Rectangle,
    pub metrics: Metrics,
    pub family: Family<'static>,
    pub color: Srgba<u8>,
    pub upscale: f32,
    pub order: isize,
    pub visible: bool,
    /// will delay text draw to next render prepare phase
    pub outdated: bool,
}

pub struct TextPipeline {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for Text {
    fn default() -> Self {
        Self {
            text: Default::default(),
            rect: Rectangle::new(0, 0, 200, 24),
            metrics: Metrics::new(24.0, 20.0),
            family: Family::SansSerif,
            color: Srgba::new(0, 0, 0, 0),
            upscale: 2.0,
            order: 100,
            visible: true,
            outdated: true,
        }
    }
}

impl Text {
    pub fn init(&mut self, world: &World, this: Handle<Text>) {
        let upscale_width = (self.rect.width() as f32 * self.upscale).ceil();
        let upscale_height = (self.rect.height() as f32 * self.upscale).ceil();

        let canvas = world.build(CanvasDescriptor {
            data: None,
            data_width: upscale_width as u32,
            data_height: upscale_height as u32,
            rect: self.rect,
            order: self.order,
            visible: self.visible,
        });

        let upscale_metrics = self.metrics.scale(self.upscale);
        let pipeline = &mut *world.single_fetch_mut::<TextPipeline>().unwrap();
        let mut buffer = Buffer::new(&mut pipeline.font_system, upscale_metrics);

        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();
                let mut canvas = world.fetch_mut(canvas).unwrap();
                let mut pipeline = world.single_fetch_mut::<TextPipeline>().unwrap();
                this.draw(
                    upscale_width,
                    upscale_height,
                    &mut buffer,
                    &mut canvas,
                    &mut pipeline,
                )
            })),
            draw: None,
        });

        RenderControl::reorder(Some(0), world, control);

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut canvas = world.fetch_mut(canvas).unwrap();
            this.visible = enabled;
            canvas.visible = enabled;
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut canvas = world.fetch_mut(canvas).unwrap();
            this.rect = rect;
            canvas.rect = rect;
            // Update canvas
        });

        self.outdated = true;
    }

    fn draw(
        &mut self,
        upscale_width: f32,
        upscale_height: f32,
        buffer: &mut Buffer,
        canvas: &mut Canvas,
        pipeline: &mut TextPipeline,
    ) -> Option<RenderInformation> {
        if !self.outdated || !self.visible {
            return None;
        }
        self.outdated = false;

        canvas.clear_transparent();

        let upscale_metrics = self.metrics.scale(self.upscale);
        let mut buffer_font = buffer.borrow_with(&mut pipeline.font_system);

        let attrs = Attrs::new().family(self.family);
        buffer_font.set_metrics(upscale_metrics);
        buffer_font.set_size(Some(upscale_width), Some(upscale_height));
        buffer_font.set_text(&self.text, &attrs, Shaping::Advanced, None);
        buffer_font.shape_until_scroll(true);

        self.draw_buffer(&buffer, canvas, pipeline);

        canvas.upload_full();

        None
    }

    fn draw_buffer(&self, buffer: &Buffer, canvas: &mut Canvas, manager: &mut TextPipeline) {
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
                                    self.color.with_alpha(image.data[i]).into_format(),
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
}

impl TextPipeline {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        let database = font_system.db_mut();

        let sans = include_bytes!("../../../fonts/SourceHanSansCN-Regular.otf").to_vec();
        let serif = include_bytes!("../../../fonts/SourceHanSerifCN-Regular.otf").to_vec();

        database.load_font_data(sans);
        database.load_font_data(serif);

        let swash_cache = SwashCache::new();

        TextPipeline {
            font_system,
            swash_cache,
        }
    }
}

impl Element for Text {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
impl Element for TextPipeline {}
