use glam::{I64Vec2, IVec2, UVec2};
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

impl BlurBrush {
    pub fn process(&self, draw: Draw) -> BlurDraw {
        BlurDraw {
            position: draw.position,
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            sigma: self.sigma.get(draw),
        }
    }

    pub fn step(&self, draw: BlurDraw) -> f32 {
        draw.size / 5.0
    }

    pub fn replace_mode(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy)]
pub struct BlurDraw {
    pub position: I64Vec2,
    pub softness: f32,
    pub size: f32,
    pub sigma: f32,
}

impl BlurDraw {
    pub fn dirty(self) -> Rectangle {
        Rectangle::new_half(
            self.position.q32_round(),
            UVec2::splat((self.size * 2.0).ceil() as u32),
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurDrawStorage {
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub sigma: f32,
    pub _pad: u32,
}

impl BlurDraw {
    pub fn into_storage(self) -> BlurDrawStorage {
        BlurDrawStorage {
            position: self.position.q32_floor(),
            position_fract: self.position.q32_fract(),
            softness: self.softness,
            size: self.size,
            sigma: self.sigma,
            _pad: 0,
        }
    }
}

impl Brush for BlurBrush {
    type BrushDraw = BlurDraw;

    type BrushDrawStorage = BlurDrawStorage;

    fn process(&self, draw: Draw) -> Self::BrushDraw {
        self.process(draw)
    }

    fn into_storage(draw: Self::BrushDraw) -> Self::BrushDrawStorage {
        draw.into_storage()
    }

    fn step(&self, draw: Self::BrushDraw) -> f32 {
        self.step(draw)
    }

    fn dirty(draw: Self::BrushDraw) -> Rectangle {
        draw.dirty()
    }

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerDrawPipeline) {
        cpass.set_pipeline(&pipeline.pipelines.blur);
    }

    fn replace_mode(&self) -> bool {
        self.replace_mode()
    }
}

impl Element for BlurBrush {}
