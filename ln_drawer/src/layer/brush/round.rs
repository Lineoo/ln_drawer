use glam::{I64Vec2, UVec2, Vec4};
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

impl RoundBrush {
    pub fn process(&self, draw: Draw) -> RoundDraw {
        RoundDraw {
            position: draw.position,
            softness: self.softness.get(draw),
            color: self.color,
            size: self.size.get(draw),
            flow: self.flow.get(draw),
        }
    }

    pub fn step(&self, draw: RoundDraw) -> f32 {
        draw.size / 5.0
    }

    pub fn replace_mode(&self) -> bool {
        self.erase
    }
}

impl Brush for RoundBrush {
    type BrushDraw = RoundDraw;

    type BrushDrawStorage = RoundDrawStorage;

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

    fn replace_mode(&self) -> bool {
        self.replace_mode()
    }

    fn set_pipeline(&self, cpass: &mut wgpu::ComputePass, pipeline: &super::LayerDrawPipeline) {
        match self.erase {
            true => cpass.set_pipeline(&pipeline.pipelines.round_erase),
            false => cpass.set_pipeline(&pipeline.pipelines.round_over),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RoundDraw {
    pub color: Srgba,
    pub position: I64Vec2,
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
}

impl RoundDraw {
    pub fn dirty(self) -> Rectangle {
        Rectangle::new_half(
            self.position.q32_round(),
            UVec2::splat((self.size * 2.0).ceil() as u32),
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RoundDrawStorage {
    pub color: Vec4,
    pub position: [i32; 2],
    pub position_fract: [u32; 2],
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
    pub _pad: u32,
}

impl RoundDraw {
    pub fn into_storage(self) -> RoundDrawStorage {
        RoundDrawStorage {
            color: Vec4::from(self.color.into_components()),
            position: self.position.q32_floor().into(),
            position_fract: self.position.q32_fract().into(),
            softness: self.softness,
            size: self.size,
            flow: self.flow,
            _pad: 0,
        }
    }
}

impl Element for RoundBrush {}
