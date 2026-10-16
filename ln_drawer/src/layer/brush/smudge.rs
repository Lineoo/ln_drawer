use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use wgpu::{BindGroup, ComputePass};

use crate::{
    brush_params,
    layer::{
        BrushKind, BrushStage, CompositeMode, LayerPipeline, WriteMode,
        brush::{
            Brush, Draw, DrawsProcessor,
            param::{BrushParam, overlap, rate_coeff, step_of},
        },
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct SmudgeBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub spacing: BrushParam<f32>,
    pub color: Vec4,
    pub color_ratio: BrushParam<f32>,
    pub sample_radius: BrushParam<f32>,
    pub sample_rate: BrushParam<f32>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SmudgeDraw {
    pub color: Vec4,
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
    pub color_ratio: f32,
    pub sample_radius: f32,
    pub sample_rate: f32,
    pub _pad: [u32; 2],
}

impl Brush for SmudgeBrush {
    fn processor(&self, from: Option<Draw>, to: Draw, out: &mut Vec<u8>) -> (Draw, Rectangle, u32) {
        self.processor_output(from, to, out)
    }

    fn prepare_stroke(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline) {
        cpass.set_pipeline(&pipeline.brush_pipelines.smudge_prepare_stroke);
        cpass.set_bind_group(0, Some(&pipeline.draws_dispatch_group), &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }

    fn prepare_draw(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline, dst: &BindGroup) {
        cpass.set_pipeline(&pipeline.brush_pipelines.smudge_prepare_draw);
        cpass.set_bind_group(0, Some(&pipeline.draws_dispatch_group), &[]);
        cpass.set_bind_group(1, Some(dst), &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        true
    }

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline, write: WriteMode) {
        cpass.set_pipeline(pipeline.brush_pipelines.get(
            BrushKind::Smudge,
            BrushStage::Draw,
            write,
            CompositeMode::Over,
        ));
    }
}

impl DrawsProcessor for SmudgeBrush {
    type Draw = SmudgeDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        let size = self.size.get(draw);
        let step = step_of(size, self.spacing.get(draw));
        let overlap = overlap(size, step);

        SmudgeDraw {
            color: self.color,
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size,
            flow: rate_coeff(self.flow.get(draw), overlap),
            color_ratio: rate_coeff(self.color_ratio.get(draw), step)
                * rate_coeff(self.sample_rate.get(draw), step),
            sample_radius: self.sample_radius.get(draw),
            sample_rate: rate_coeff(self.sample_rate.get(draw), step),
            _pad: [0; 2],
        }
    }

    fn step(&self, draw: Draw) -> f32 {
        step_of(self.size.get(draw), self.spacing.get(draw))
    }

    fn dirty(&self, draw: Self::Draw) -> Rectangle {
        Rectangle::new_half(
            draw.position,
            UVec2::splat(((draw.size * 2.0).ceil() as u32) + 1),
        )
    }
}

brush_params!(SmudgeBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
    Color => Color(color),
    ColorRatio => Scalar(color_ratio),
    SampleRadius => Scalar(sample_radius),
    SampleRate => Scalar(sample_rate),
});

impl Element for SmudgeBrush {}
