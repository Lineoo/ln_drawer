use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::{
        LayerPipeline,
        brush::{Brush, Draw, param::BrushParam},
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct TintBrush {
    pub size: BrushParam<f32>,
    pub softness: BrushParam<f32>,
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
        TintDraw {
            color: Vec4::from(self.color.into_components()),
            flow: self.flow,
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            _pad: [0; 2],
        }
    }

    fn step(&self, draw: Self::Draw) -> f32 {
        draw.size / 5.0
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
