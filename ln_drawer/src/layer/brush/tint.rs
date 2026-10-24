use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;

use crate::{
    brush_params,
    layer::{
        BrushKind, BrushStage, CompositeMode, LayerPipeline, WriteMode,
        brush::{
            Brush, Draw, DrawsProcessor,
            param::{BrushParam, overlap, rate_coeff, step_of},
        },
    },
    measures::{FI64Ext, IRect},
};

#[derive(Clone)]
pub struct TintBrush {
    pub size: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub spacing: BrushParam<f32>,
    pub color: Vec4,
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
    fn processor(&self, from: Option<Draw>, to: Draw, out: &mut Vec<u8>) -> (Draw, IRect, u32) {
        self.processor_output(from, to, out)
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        false
    }

    fn set_pipeline(
        &self,
        cpass: &mut wgpu::ComputePass,
        pipeline: &LayerPipeline,
        write: WriteMode,
    ) {
        cpass.set_pipeline(pipeline.brush_pipelines.get(
            BrushKind::Tint,
            BrushStage::Draw,
            write,
            CompositeMode::Over,
        ));
    }
}

impl DrawsProcessor for TintBrush {
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
            color: self.color,
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

    fn dirty(&self, draw: Self::Draw) -> IRect {
        IRect::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }
}

brush_params!(TintBrush, {
    Size => Scalar(size),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
    Color => Color(color),
    Flow => Vec4(flow),
});

impl Element for TintBrush {}
