use glam::{DVec2, I64Vec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor};

use crate::{
    color::{oklab::oklab_to_linear_srgb, srgb::srgb_gamma_encode},
    layer::{
        LayerPipeline, Standalone,
        brush::{BrushParams, Draw, DrawsBatch, Stroke, param::BrushParam, round::RoundBrush},
        wrapper::LayerPage,
    },
    layout::val::{Val, ValChanged},
    measures::{FI64Ext, IRect},
    render::{RenderDrawControl, RenderPrepareControl, camera::SetCameraOutdated},
    tools::collider::ToolRectCollider,
    widgets::{
        common::WidgetCommon,
        container::{ContainerAttr, ContainerEcho, ContainerWithCamera},
    },
};

/// A widget that displays an engine-rendered brush preview.
///
/// Set [`BrushPreview::outdated`] and the preview regenerates with [`BrushPreview::brush`] on the
/// next frame.
pub struct BrushPreview {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
    pub brush: Option<Box<dyn BrushParams>>,
    pub outdated: bool,
}

struct BrushPreviewInstance {
    canvas: Standalone,
}

impl BrushPreview {
    fn init(&self, world: &World, this: Handle<Self>) {
        world.scope("brush_preview");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let page = world.single_fetch::<LayerPage>();
        let rect = world.fetch(self.rect).val;

        let canvas_rect = IRect::new_extend(0, 0, rect.width().max(1), rect.height().max(1));
        let canvas = page.lp.create_standalone(canvas_rect);

        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr: ContainerAttr {
                anchor: DVec2::new(0.0, 0.0),
                ..Default::default()
            },
        });
        let inner = world.build(ContainerEcho { container });
        world.queue_trigger(common.camera, SetCameraOutdated);

        world.insert(ToolRectCollider {
            common,
            target: this.untyped(),
            rect: inner,
            order: 0,
        });

        let instance = world.insert(BrushPreviewInstance { canvas });

        world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let mut this = world.fetch_mut(this);
                let instance = world.fetch(instance);
                let visible = world.fetch(this.common.visible).val;
                if let Some(brush) = &this.brush
                    && this.outdated
                    && visible
                {
                    let page = world.single_fetch::<LayerPage>();
                    Self::generate_preview(&page.lp, &instance.canvas, brush.as_ref());
                    this.outdated = false;
                }
                None
            }),
        });

        world.insert(RenderDrawControl {
            common,
            order: 0,
            draw: Box::new(move |world, rpass, extra| {
                let instance = world.fetch(instance);
                let page = world.single_fetch::<LayerPage>();
                page.lp
                    .render_chunk(rpass, extra.camera, &instance.canvas.chunk);
            }),
        });

        world.observer(self.rect, move |ValChanged, world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;
            let mut instance = world.fetch_mut(instance);

            let size = UVec2::new(rect.width().max(1), rect.height().max(1));
            if size != instance.canvas.rect.extend {
                let page = world.single_fetch::<LayerPage>();
                instance.canvas = page
                    .lp
                    .create_standalone(IRect::new_extend(0, 0, size.x, size.y));
                this.outdated = true;
            }

            world.queue_trigger(common.camera, SetCameraOutdated);
        });
    }

    /// Redraw `brush` over a fixed seed into `canvas`.
    pub fn generate_preview(lp: &LayerPipeline, canvas: &Standalone, brush: &dyn BrushParams) {
        let swap = canvas
            .chunk
            .read_write
            .is_none()
            .then(|| lp.create_standalone(canvas.rect));

        let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("preview_paint_clear"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("preview_paint_clear"),
            timestamp_writes: None,
        });

        lp.clear_chunk_direct(&mut cpass, &canvas.chunk, canvas.rect);

        drop(cpass);
        lp.queue.submit([encoder.finish()]);

        if brush.replace_mode() {
            Self::draw_background(lp, canvas, &swap);
        }

        Self::draw_stroke(lp, canvas, &swap, brush);
    }

    fn submit_batch(
        lp: &LayerPipeline,
        canvas: &Standalone,
        swap: &Option<Standalone>,
        brush: &dyn BrushParams,
        batch: &DrawsBatch,
    ) {
        batch.upload_dispatch(lp);

        let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("preview_draw"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("preview_draw"),
            timestamp_writes: None,
        });

        batch.draw_into_standalone(lp, &mut cpass, canvas, swap, brush);

        drop(cpass);
        lp.queue.submit([encoder.finish()]);
    }

    /// Two vertical bands, one red on the left and one blue on the right, each half the canvas
    /// wide, used as the preview's background.
    fn draw_background(lp: &LayerPipeline, canvas: &Standalone, swap: &Option<Standalone>) {
        let w = canvas.rect.width() as f32;
        let h = canvas.rect.height() as f32;
        let radius = w / 4.0 + 2.5;
        let c1 = srgb_gamma_encode(oklab_to_linear_srgb(Vec4::new(0.98, 0.1, 0.02, 1.0)));
        let c2 = srgb_gamma_encode(oklab_to_linear_srgb(Vec4::new(0.98, -0.1, 0.02, 1.0)));
        let bands = [(w * 0.25, c1), (w * 0.75, c2)];

        for (x, color) in bands {
            let brush = RoundBrush {
                size: BrushParam::constant(radius),
                flow: BrushParam::constant(1.0),
                softness: BrushParam::constant(0.01),
                spacing: BrushParam::constant(0.1),
                color,
                erase: false,
            };

            let start = Draw {
                position: I64Vec2::q32_from_f64(DVec2::new(x as f64, -radius as f64)),
                force: 1.0,
            };

            let end = Draw {
                position: I64Vec2::q32_from_f64(DVec2::new(x as f64, (h + radius) as f64)),
                force: 1.0,
            };

            let mut stroke = Stroke::new(&brush);

            let Some(first) = stroke.batch(&brush, start) else {
                continue;
            };

            stroke.confirm_batch(&first);

            let Some(last) = stroke.batch(&brush, end) else {
                Self::submit_batch(lp, canvas, swap, &brush, &first);
                continue;
            };

            stroke.confirm_batch(&last);

            let (merged, rest) = first.merge(last);
            Self::submit_batch(lp, canvas, swap, &brush, &merged);
            if let Some(rest) = rest {
                Self::submit_batch(lp, canvas, swap, &brush, &rest);
            }
        }
    }

    /// A gentle S-curve with a pressure ramp, drawn with the previewed brush.
    fn draw_stroke(
        lp: &LayerPipeline,
        canvas: &Standalone,
        swap: &Option<Standalone>,
        brush: &dyn BrushParams,
    ) {
        const STEPS: i32 = 24;

        let mut stroke = Stroke::new(brush);
        let mut batches = Vec::new();

        for i in 0..=STEPS {
            let t = i as f32 / STEPS as f32;
            let x = canvas.rect.width() as f32 * 0.5 - 125. + 250. * t;
            let y = canvas.rect.height() as f32 * 0.5 + 10. * (t * std::f32::consts::TAU).sin();
            let f = 4. * t * (1. - t);
            let draw = Draw {
                position: I64Vec2::q32_from_f64(DVec2::new(x as f64, y as f64)),
                force: 0.25 + 0.75 * f,
            };

            let Some(batch) = stroke.batch(brush, draw) else {
                break;
            };

            stroke.confirm_batch(&batch);
            batches.push(batch);
        }

        let mut merged = Vec::new();

        for batch in batches {
            let Some(prev) = merged.pop() else {
                merged.push(batch);
                continue;
            };

            let (prev, rest) = prev.merge(batch);
            merged.push(prev);
            if let Some(rest) = rest {
                merged.push(rest);
            }
        }

        for batch in &merged {
            Self::submit_batch(lp, canvas, swap, brush, batch);
        }
    }
}

impl Element for BrushPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for BrushPreviewInstance {}
