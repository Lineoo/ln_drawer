use glam::{IVec2, UVec2};
use ln_world::Element;
use wgpu::ComputePass;

use crate::{
    layer::brush::{Brush, Draw, LayerDrawPipeline, param::BrushParam},
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct BlurBrush {
    pub size: BrushParam<f32>,
    pub sigma: BrushParam<f32>,
    pub softness: BrushParam<f32>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurDraw {
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub sigma: f32,
    pub _pad: u32,
}

impl Brush for BlurBrush {
    type Draw = BlurDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        BlurDraw {
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            sigma: self.sigma.get(draw),
            _pad: 0,
        }
    }

    fn step(&self, draw: Self::Draw) -> f32 {
        draw.size / 5.0
    }

    fn dirty(draw: Self::Draw) -> Rectangle {
        Rectangle::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        true
    }

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerDrawPipeline) {
        cpass.set_pipeline(&pipeline.pipelines.blur);
    }
}

impl Element for BlurBrush {}
