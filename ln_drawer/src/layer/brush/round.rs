use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::brush::{Brush, Draw, param::BrushParam},
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct RoundBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub color: Srgba,
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
    type BrushDraw = RoundDraw;

    fn process(&self, draw: Draw) -> Self::BrushDraw {
        RoundDraw {
            color: Vec4::from(self.color.into_components()),
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            flow: self.flow.get(draw),
            _pad: 0,
        }
    }

    fn step(&self, draw: Self::BrushDraw) -> f32 {
        draw.size / 5.0
    }

    fn dirty(draw: Self::BrushDraw) -> Rectangle {
        Rectangle::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }

    fn set_pipeline(&self, cpass: &mut wgpu::ComputePass, pipeline: &super::LayerDrawPipeline) {
        match self.erase {
            true => cpass.set_pipeline(&pipeline.pipelines.round_erase),
            false => cpass.set_pipeline(&pipeline.pipelines.round_over),
        }
    }

    fn replace_mode(&self) -> bool {
        self.erase
    }
}

impl Element for RoundBrush {}
