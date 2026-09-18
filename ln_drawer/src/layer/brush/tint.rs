use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::{
        LayerPipeline,
        brush::{
            Brush, Draw,
            param::{BrushParam, rate_coeff, overlap, step_of},
        },
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct TintBrush {
    pub size: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub spacing: BrushParam<f32>,
    pub color: Srgba,
    pub flow: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TintDraw {
    pub color: Vec4,
    pub flow: Vec4,
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub _pad: [u32; 2],
}

impl Brush for TintBrush {
    type Draw = TintDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        let size = self.size.get(draw);
        let step = step_of(size, self.spacing.get(draw));
        let overlap = overlap(size, step);

        // `flow.a` is the per-dab alpha and `flow.rgb` the under/over mix ratio; both accumulate
        // geometrically so both are normalized by the overlap count.
        let flow = Vec4::new(
            rate_coeff(self.flow.x, overlap),
            rate_coeff(self.flow.y, overlap),
            rate_coeff(self.flow.z, overlap),
            rate_coeff(self.flow.w, overlap),
        );

        TintDraw {
            color: Vec4::from(self.color.into_components()),
            flow,
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size,
            _pad: [0; 2],
        }
    }

    fn step(&self, draw: Draw) -> f32 {
        step_of(self.size.get(draw), self.spacing.get(draw))
    }

    fn dirty(&self, draw: Self::Draw) -> Rectangle {
        Rectangle::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        false
    }

    fn set_pipeline(&self, cpass: &mut wgpu::ComputePass, pipeline: &LayerPipeline) {
        cpass.set_pipeline(&pipeline.brush_pipelines.tint);
    }
}

impl Element for TintBrush {}
