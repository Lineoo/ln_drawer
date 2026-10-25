use glam::{DVec2, I64Vec2, UVec2, Vec4};
use ln_world::{Descriptor, Element, Handle, HandleGeneric, World};
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, Extent3d, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect,
};

use crate::{
    color::srgb::{mul_alpha, srgb_gamma_decode, srgb_gamma_encode},
    layer::{
        LayerPipeline, LayerRenderer, Standalone,
        brush::{BrushParams, Draw, DrawsBatch, Stroke},
        wrapper::LayerPage,
    },
    layout::{
        transfer::Transfer,
        val::{Val, ValChanged},
    },
    measures::{FI64Ext, IRect},
    render::{RenderDrawControl, RenderPrepareControl, camera::SetCameraOutdated},
    tools::collider::ToolRectCollider,
    widgets::{
        common::WidgetCommon,
        container::{ContainerAttr, ContainerEcho, ContainerWithCamera},
    },
};

pub struct BrushPreviewDescriptor {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
    pub background: Handle<Val<Vec4>>,
    /// Brush preview band colors, in L.
    pub c0: Handle<Val<Vec4>>,
    pub c1: Handle<Val<Vec4>>,
    // TODO only for brush list, not for setting preview
    /// Brush stroke color, in L.
    pub bc: Handle<Val<Vec4>>,
    pub brush: Box<dyn BrushParams>,
}

/// A widget that displays an engine-rendered brush preview.
pub struct BrushPreview {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
    pub background: Handle<Val<Vec4>>,
    /// Brush preview band colors, in S(L).
    pub c0: Handle<Val<Vec4>>,
    pub c1: Handle<Val<Vec4>>,
    /// Brush stroke color, in S(L).
    pub bc: Handle<Val<Vec4>>,
    pub brush: Box<dyn BrushParams>,
    pub canvas: Standalone,
    pub renderer: LayerRenderer,
    pub outdated: bool,
}

impl Descriptor for BrushPreviewDescriptor {
    type Target = BrushPreview;

    fn when_build(self, world: &World) -> Self::Target {
        let page = world.single_fetch::<LayerPage>();
        let renderer = page.lp.create_render(world.fetch(self.background).val);
        let canvas = page.lp.create_standalone(world.fetch(self.rect).val);
        let c0 = Transfer::derive(world, self.c0).build(|&c| srgb_gamma_encode(c));
        let c1 = Transfer::derive(world, self.c1).build(|&c| srgb_gamma_encode(c));
        let bc = Transfer::derive(world, self.bc).build(|&c| srgb_gamma_encode(c));
        BrushPreview {
            common: self.common,
            rect: self.rect,
            background: self.background,
            c0,
            c1,
            bc,
            brush: self.brush,
            canvas,
            renderer,
            outdated: false,
        }
    }
}

impl Element for BrushPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("brush_preview");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

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

        world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let mut this = world.fetch_mut(this);
                let visible = world.fetch(this.common.visible).val;
                if this.outdated && visible {
                    let page = world.single_fetch::<LayerPage>();
                    let c0 = world.fetch(this.c0).val;
                    let c1 = world.fetch(this.c1).val;
                    let bc = world.fetch(this.bc).val;
                    let BrushPreview { canvas, brush, .. } = &mut *this;
                    Self::generate_preview(&page.lp, canvas, brush.as_mut(), c0, c1, bc);
                    this.outdated = false;
                }
                None
            }),
        });

        world.insert(RenderDrawControl {
            common,
            order: 0,
            draw: Box::new(move |world, rpass, extra| {
                let this = world.fetch(this);
                this.renderer
                    .render_chunk(rpass, extra.camera, &this.canvas.chunk);
            }),
        });

        world.observer(self.rect, move |ValChanged, world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;

            let size = UVec2::new(rect.width(), rect.height());
            if size != this.canvas.rect.extend {
                let page = world.single_fetch::<LayerPage>();
                this.canvas = page
                    .lp
                    .create_standalone(IRect::new_extend(0, 0, size.x, size.y));
                this.outdated = true;
            }

            world.queue_trigger(common.camera, SetCameraOutdated);
        });

        world.observer(self.background, move |ValChanged, world| {
            let this = world.fetch(this);
            let background_color = world.fetch(this.background).val;
            this.renderer.write_background(background_color);
        });

        world.observer(self.c0, move |ValChanged, world| {
            world.fetch_mut(this).outdated = true;
        });

        world.observer(self.c1, move |ValChanged, world| {
            world.fetch_mut(this).outdated = true;
        });

        world.observer(self.bc, move |ValChanged, world| {
            world.fetch_mut(this).outdated = true;
        });
    }
}

impl BrushPreview {
    /// Redraw `brush` over a fixed seed into `canvas`.
    pub fn generate_preview(
        lp: &LayerPipeline,
        canvas: &Standalone,
        brush: &mut dyn BrushParams,
        c0: Vec4,
        c1: Vec4,
        distinct: Vec4,
    ) {
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
            Self::draw_background(lp, canvas, c0, c1);
        }

        brush.set_color(distinct);
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

    /// Two differently colored halves, left and right, as the preview background.
    fn draw_background(lp: &LayerPipeline, canvas: &Standalone, c0: Vec4, c1: Vec4) {
        let size = canvas.rect.extend;
        if size.x == 0 || size.y == 0 {
            return;
        }

        // Brush colors are S(L), while the canvas stores S(P(L)).
        let texel = |color: Vec4| {
            let color = srgb_gamma_encode(mul_alpha(srgb_gamma_decode(color)));
            [
                (color.x * 255.0).clamp(0.0, 255.0).round() as u8,
                (color.y * 255.0).clamp(0.0, 255.0).round() as u8,
                (color.z * 255.0).clamp(0.0, 255.0).round() as u8,
                (color.w * 255.0).clamp(0.0, 255.0).round() as u8,
            ]
        };

        let left = texel(c0);
        let right = texel(c1);
        let split = size.x / 2;

        let mut row = Vec::with_capacity((size.x * 4) as usize);
        for x in 0..size.x {
            row.extend_from_slice(if x < split { &left } else { &right });
        }

        let mut data = Vec::with_capacity(row.len() * size.y as usize);
        for _ in 0..size.y {
            data.extend_from_slice(&row);
        }

        lp.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &canvas.chunk.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.x * 4),
                rows_per_image: Some(size.y),
            },
            Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
        );
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
