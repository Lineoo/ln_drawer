use glam::{DVec2, I64Vec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor};

use crate::{
    color::{oklab::oklab_to_linear_srgb, srgb::srgb_gamma_encode},
    layer::{
        LayerPipeline, Standalone,
        brush::{BrushParams, Draw, Stroke, param::BrushParam, round::RoundBrush},
        wrapper::LayerPage,
    },
    measures::{FI64Ext, Rectangle},
    render::{
        RenderDrawControl, RenderPrepareControl,
        camera::{Camera, CameraDescriptor, SetCameraOutdated},
    },
    tools::collider::ToolRectCollider,
    widgets::{SetWidgetRectangle, SetWidgetVisible, common::WidgetCommon, container::Container},
};

/// A widget that displays an engine-rendered brush preview.
///
/// Set [`BrushPreview::outdated`] and the preview regenerates with [`BrushPreview::brush`] on the
/// next frame.
pub struct BrushPreview {
    pub rect: Rectangle,
    pub brush: Option<Box<dyn BrushParams>>,
    pub outdated: bool,
    pub visible: bool,
    pub camera: Handle<Camera>,
}

struct BrushPreviewInstance {
    canvas: Standalone,
    container: Handle<Container>,
    collider: Handle<ToolRectCollider>,
}

impl BrushPreview {
    fn init(&self, world: &World, this: Handle<Self>) {
        let page = world.single_fetch::<LayerPage>();
        let canvas = page.lp.create_standalone(Rectangle::new_extend(
            0,
            0,
            self.rect.width().max(1),
            self.rect.height().max(1),
        ));

        // The chunk always lives at `preview_rect`, so the container's child camera is the only
        // thing that has to follow the widget.
        let parent = world.fetch(self.camera);
        let camera = world.insert(world.build(CameraDescriptor {
            center: canvas.rect.q32_center(),
            zoom: parent.zoom,
        }));
        drop(parent);

        let container = world.insert(Container {
            common: WidgetCommon {
                camera: self.camera,
                visible: self.visible,
            },
            order: 50,
            child: camera,
            rect: self.rect,
            anchor: DVec2::splat(0.5),
            scroll: I64Vec2::ZERO,
        });
        world.queue_trigger(camera, SetCameraOutdated);

        let collider = world.insert(ToolRectCollider {
            common: WidgetCommon {
                camera,
                visible: self.visible,
            },
            target: this.untyped(),
            rect: canvas.rect,
            order: 0,
        });

        let instance = world.insert(BrushPreviewInstance {
            canvas,
            container,
            collider,
        });

        let prepare = world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let mut this = world.fetch_mut(this);
                let instance = world.fetch(instance);
                if let Some(brush) = &this.brush
                    && this.outdated
                    && this.visible
                {
                    let page = world.single_fetch::<LayerPage>();
                    Self::generate_preview(&page.lp, &instance.canvas, brush.as_ref());
                    this.outdated = false;
                }
                None
            }),
        });

        world.insert(RenderDrawControl {
            common: WidgetCommon {
                camera,
                visible: true,
            },
            order: 0,
            draw: Box::new(move |world, rpass, extra| {
                let instance = world.fetch(instance);
                let page = world.single_fetch::<LayerPage>();
                page.lp
                    .render_chunk(rpass, extra.camera, &instance.canvas.chunk);
            }),
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            let mut instance = world.fetch_mut(instance);

            let size = UVec2::new(rect.width().max(1), rect.height().max(1));
            if size != instance.canvas.rect.extend {
                let page = world.single_fetch::<LayerPage>();
                instance.canvas = page
                    .lp
                    .create_standalone(Rectangle::new_extend(0, 0, size.x, size.y));
                world.fetch_mut(camera).center = instance.canvas.rect.q32_center();
                this.outdated = true;
            }

            this.rect = rect;
            world.queue_trigger(instance.container, SetWidgetRectangle(rect));
            world.queue_trigger(instance.collider, SetWidgetRectangle(instance.canvas.rect));
            world.queue_trigger(camera, SetCameraOutdated);
        });

        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this);
            this.visible = visible;
            let instance = world.fetch(instance);
            world.queue_trigger(instance.container, SetWidgetVisible(visible));
            world.queue_trigger(instance.collider, SetWidgetVisible(visible));
        });

        world.dependency(prepare, this);
        world.dependency(container, this);
        world.dependency(instance, this);
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

            // first

            let Some(batch) = stroke.batch(&brush, start) else {
                return;
            };

            stroke.confirm_batch(&batch);
            batch.upload_dispatch(lp);

            let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("preview_draw_background"),
            });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("preview_draw_background"),
                timestamp_writes: None,
            });

            batch.draw_into_standalone(lp, &mut cpass, canvas, swap, &brush);

            drop(cpass);

            lp.queue.submit([encoder.finish()]);

            // end

            let Some(batch) = stroke.batch(&brush, end) else {
                return;
            };

            stroke.confirm_batch(&batch);
            batch.upload_dispatch(lp);

            let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("preview_draw_background"),
            });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("preview_draw_background"),
                timestamp_writes: None,
            });

            batch.draw_into_standalone(lp, &mut cpass, canvas, swap, &brush);

            drop(cpass);

            lp.queue.submit([encoder.finish()]);
        }
    }

    /// A gentle S-curve with a pressure ramp, drawn with the previewed brush.
    fn draw_stroke(
        lp: &LayerPipeline,
        canvas: &Standalone,
        swap: &Option<Standalone>,
        brush: &dyn BrushParams,
    ) {
        const STEPS: i32 = 8;

        let mut stroke = Stroke::new(brush);

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
                return;
            };

            stroke.confirm_batch(&batch);
            batch.upload_dispatch(&lp);

            let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
                label: Some("preview_draw_stroke"),
            });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("preview_draw_stroke"),
                timestamp_writes: None,
            });

            batch.draw_into_standalone(&lp, &mut cpass, canvas, swap, brush);

            drop(cpass);
            lp.queue.submit([encoder.finish()]);
        }
    }
}

impl Element for BrushPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for BrushPreviewInstance {}
