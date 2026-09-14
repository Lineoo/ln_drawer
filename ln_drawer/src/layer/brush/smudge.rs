use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;
use wgpu::{BindGroup, ComputePass};

use crate::{
    layer::{
        LayerPipeline,
        brush::{Brush, Draw, param::BrushParam},
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct SmudgeBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub color: Srgba,
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
    type Draw = SmudgeDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        SmudgeDraw {
            color: Vec4::from(self.color.into_components()),
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            flow: self.flow.get(draw),
            color_ratio: self.color_ratio.get(draw),
            sample_radius: self.sample_radius.get(draw),
            sample_rate: self.sample_rate.get(draw),
            _pad: [0; 2],
        }
    }

    fn step(&self, draw: Self::Draw) -> f32 {
        draw.size / 5.0
    }

    fn dirty(&self, draw: Self::Draw) -> Rectangle {
        Rectangle::new_half(
            draw.position,
            UVec2::splat(((draw.size * 2.0).ceil() as u32) + 1),
        )
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        true
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

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline) {
        cpass.set_pipeline(&pipeline.brush_pipelines.smudge);
    }
}

impl Element for SmudgeBrush {}
