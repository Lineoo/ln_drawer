use std::sync::Arc;

use glam::{DVec2, I64Vec2};
use ln_world::{Element, Handle, World};
use palette::Srgba;
use wgpu::TextureViewDescriptor;

use crate::{
    layer::{
        LayerPipeline, Standalone,
        brush::{BrushParams, Draw, DrawPipeline, param::BrushParam, round::RoundBrush},
        wrapper::LayerWrapper,
    },
    measures::{FI64Ext, Rectangle},
    render::RenderControl,
    widgets::{SetWidgetRectangle, SetWidgetVisible, renderer::texture_quad::TextureQuad},
};

/// A widget that displays an engine-rendered brush preview.
///
/// It is agnostic of where the brush comes from: callers invoke [`BrushPreview::paint`] from
/// their own observers when the brush or its settings change.
pub struct BrushPreview {
    pub rect: Rectangle,
    pub generator: Handle<BrushPreviewGenerator>,
    pub brush: Option<Box<dyn BrushParams>>,
    pub outdated: bool,
}

struct BrushPreviewInstance {
    quad: Handle<TextureQuad>,
    preview_canvas: Standalone,
}

/// Bakes a real brush stroke into a small offscreen [`Standalone`] canvas.
///
/// Uses a dedicated [`DrawPipeline`] so the live stroke and undo state of the drawing engine are
/// never touched, and draws into the canvas directly through [`DrawPipeline::draw_standalone`]
/// instead of the scratch layer.
pub struct BrushPreviewGenerator {
    pub layer: Arc<LayerPipeline>,
    pub pipe: DrawPipeline,
}

impl BrushPreview {
    fn init(&self, world: &World, this: Handle<Self>) {
        let generator_instance = world.fetch(self.generator).unwrap();
        let preview_canvas = generator_instance
            .layer
            .create_standalone(Rectangle::new_extend(
                0,
                0,
                self.rect.width().max(1),
                self.rect.height().max(1),
            ));
        let view = preview_canvas
            .chunk
            .texture
            .create_view(&TextureViewDescriptor::default());
        let quad = world.insert(TextureQuad {
            rect: Rectangle::default(),
            visible: true,
            order: 50,
            view,
        });
        let instance = world.insert(BrushPreviewInstance {
            quad,
            preview_canvas,
        });
        world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();
                let instance = world.fetch(instance).unwrap();
                if let Some(brush) = &this.brush
                    && this.outdated
                {
                    let mut generator = world.fetch_mut(this.generator).unwrap();
                    generator.paint(&instance.preview_canvas, brush.as_ref());
                    this.outdated = false;
                }
                None
            })),
            draw: None,
        });
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut instance = world.fetch_mut(instance).unwrap();
            if rect.extend != instance.preview_canvas.rect.extend {
                let quad = world.fetch(instance.quad).unwrap();
                let old_visible = quad.visible;
                drop(quad);
                world.remove(instance.quad).unwrap();
                let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
                instance.preview_canvas = wrapper.brush.layer.create_standalone(
                    Rectangle::new_extend(0, 0, rect.width().max(1), rect.height().max(1)),
                );
                this.outdated = true;
                let view = instance
                    .preview_canvas
                    .chunk
                    .texture
                    .create_view(&TextureViewDescriptor::default());
                let quad = world.insert(TextureQuad {
                    rect,
                    visible: old_visible,
                    order: 50,
                    view,
                });
                instance.quad = quad;
            }
            this.rect = rect;
            world.queue_trigger(instance.quad, SetWidgetRectangle(rect));
        });
        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let instance = world.fetch(instance).unwrap();
            world.queue_trigger(instance.quad, SetWidgetVisible(visible));
        });
    }
}

impl BrushPreviewGenerator {
    pub fn new(layer: Arc<LayerPipeline>) -> Self {
        BrushPreviewGenerator {
            pipe: DrawPipeline::new(layer.clone()),
            layer,
        }
    }

    /// Redraw `brush` over a fixed seed into `canvas`.
    pub fn paint(&mut self, canvas: &Standalone, brush: &dyn BrushParams) {
        self.layer.clear_chunk(&canvas.chunk, canvas.rect);

        self.pipe.reset();
        self.paint_seed(canvas);
        // Begin the brush stroke without connecting it to the seed path.
        self.pipe.reset();
        self.paint_stroke(canvas, brush);
    }

    /// Two vertical bands, one red on the left and one blue on the right, each half the canvas
    /// wide, used as the preview's background.
    fn paint_seed(&mut self, canvas: &Standalone) {
        let w = canvas.rect.width() as f32;
        let h = canvas.rect.height() as f32;
        let radius = w / 4.0;
        let bands = [
            (w * 0.25, Srgba::new(0.90, 0.20, 0.20, 1.0)),
            (w * 0.75, Srgba::new(0.25, 0.40, 0.95, 1.0)),
        ];

        for (x, color) in bands {
            let brush = RoundBrush {
                size: BrushParam::constant(radius),
                flow: BrushParam::constant(0.9),
                softness: BrushParam::constant(0.01),
                color,
                erase: false,
            };

            // Reset between bands so the interpolation never bridges them with a diagonal.
            self.pipe.reset();
            self.pipe.draw_standalone(
                canvas,
                &brush,
                Draw {
                    position: I64Vec2::q32_from_f64(DVec2::new(x as f64, -radius as f64)),
                    force: 1.0,
                },
            );
            self.pipe.draw_standalone(
                canvas,
                &brush,
                Draw {
                    position: I64Vec2::q32_from_f64(DVec2::new(x as f64, (h + radius) as f64)),
                    force: 1.0,
                },
            );
        }
    }

    /// A gentle S-curve with a pressure ramp, drawn with the previewed brush.
    fn paint_stroke(&mut self, canvas: &Standalone, brush: &dyn BrushParams) {
        let steps = 24;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = canvas.rect.width() as f32 * (0.18 + 0.64 * t);
            let y = canvas.rect.height() as f32 * (0.5 + 0.28 * (t * std::f32::consts::TAU).sin());
            let f = 4. * t * (1. - t);

            brush.draw_standalone(
                &mut self.pipe,
                canvas,
                Draw {
                    position: I64Vec2::q32_from_f64(DVec2::new(x as f64, y as f64)),
                    force: 0.25 + 0.75 * f,
                },
            );
        }
    }
}

impl Element for BrushPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for BrushPreviewInstance {}
impl Element for BrushPreviewGenerator {}
