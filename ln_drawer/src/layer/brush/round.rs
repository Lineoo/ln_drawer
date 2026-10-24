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
pub struct RoundBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub spacing: BrushParam<f32>,
    pub color: Vec4,
    pub erase: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RoundDraw {
    pub color: Vec4,
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
    pub _pad: u32,
}

impl Brush for RoundBrush {
    fn processor(&self, from: Option<Draw>, to: Draw, out: &mut Vec<u8>) -> (Draw, IRect, u32) {
        self.processor_output(from, to, out)
    }

    fn replace_mode(&self) -> bool {
        self.erase
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
        let composite = match self.erase {
            true => CompositeMode::Erase,
            false => CompositeMode::Over,
        };
        cpass.set_pipeline(pipeline.brush_pipelines.get(
            BrushKind::Round,
            BrushStage::Draw,
            write,
            composite,
        ));
    }
}

impl DrawsProcessor for RoundBrush {
    type Draw = RoundDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        let size = self.size.get(draw);
        let step = step_of(size, self.spacing.get(draw));
        let overlap = overlap(size, step);

        RoundDraw {
            color: self.color,
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size,
            flow: rate_coeff(self.flow.get(draw), overlap),
            _pad: 0,
        }
    }

    fn step(&self, draw: Draw) -> f32 {
        step_of(self.size.get(draw), self.spacing.get(draw))
    }

    fn dirty(&self, draw: Self::Draw) -> IRect {
        IRect::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }
}

brush_params!(RoundBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
    Color => Color(color),
    Erase => Toggle(erase),
});

impl Element for RoundBrush {}
