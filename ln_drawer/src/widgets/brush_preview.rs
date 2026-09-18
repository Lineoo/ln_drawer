use std::sync::Arc;

use glam::{DVec2, I64Vec2, IVec2};
use ln_world::{Element, Handle, World};
use palette::Srgba;
use wgpu::TextureViewDescriptor;

use crate::{
    layer::{
        DEFAULT_CHUNK_SIZE, LayerPipeline, Standalone,
        brush::{BrushParams, Draw, DrawPipeline, param::BrushParam, round::RoundBrush},
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
    pub canvas: Standalone,
    pub generator: Handle<BrushPreviewGenerator>,
    pub brush: Option<Box<dyn BrushParams>>,
    pub outdated: bool,
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
        let view = self
            .canvas
            .chunk
            .texture
            .create_view(&TextureViewDescriptor::default());
        let quad = world.insert(TextureQuad {
            rect: Rectangle::default(),
            visible: true,
            order: 50,
            view,
        });
        world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();
                if let Some(brush) = &this.brush
                    && this.outdated
                {
                    let mut generator = world.fetch_mut(this.generator).unwrap();
                    generator.paint(&this.canvas, brush.as_ref());
                    this.outdated = false;
                }
                None
            })),
            draw: None,
        });
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });
        world.observer(this, move |&SetWidgetVisible(visible), world| {
            world.queue_trigger(quad, SetWidgetVisible(visible));
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

    /// A serpentine of coloured bands filling the canvas, used as the preview's background.
    fn paint_seed(&mut self, canvas: &Standalone) {
        let c = DEFAULT_CHUNK_SIZE as i32;
        let margin = c / 8;
        let colors = [
            Srgba::new(0.90, 0.20, 0.20, 1.0),
            Srgba::new(0.20, 0.80, 0.30, 1.0),
            Srgba::new(0.25, 0.40, 0.95, 1.0),
            Srgba::new(0.95, 0.85, 0.20, 1.0),
        ];
        let path = [
            IVec2::new(margin, margin),
            IVec2::new(c - margin, margin),
            IVec2::new(c - margin, c / 2),
            IVec2::new(margin, c / 2),
            IVec2::new(margin, c - margin),
            IVec2::new(c - margin, c - margin),
        ];

        for i in 0..path.len() - 1 {
            let brush = RoundBrush {
                size: BrushParam::constant(DEFAULT_CHUNK_SIZE as f32 * 0.22),
                flow: BrushParam::constant(0.9),
                softness: BrushParam::constant(0.01),
                color: colors[i % colors.len()],
                erase: false,
            };

            if i == 0 {
                self.pipe.draw_standalone(
                    canvas,
                    &brush,
                    Draw {
                        position: I64Vec2::q32_from_i32(path[0]),
                        force: 1.0,
                    },
                );
            }
            self.pipe.draw_standalone(
                canvas,
                &brush,
                Draw {
                    position: I64Vec2::q32_from_i32(path[i + 1]),
                    force: 1.0,
                },
            );
        }
    }

    /// A gentle S-curve with a pressure ramp, drawn with the previewed brush.
    fn paint_stroke(&mut self, canvas: &Standalone, brush: &dyn BrushParams) {
        let c = DEFAULT_CHUNK_SIZE as f32;
        let steps = 24;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = c * (0.18 + 0.64 * t);
            let y = c * (0.5 + 0.28 * (t * std::f32::consts::TAU).sin());
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

impl Element for BrushPreviewGenerator {}
