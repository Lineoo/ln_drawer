use glam::{IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::{
        LayerPipeline,
        brush::{
            Brush, Draw,
            param::{BrushParam, flow_coeff, overlap, step_of},
        },
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct RoundBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub softness: BrushParam<f32>,
    pub spacing: BrushParam<f32>,
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
    type Draw = RoundDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        let size = self.size.get(draw);
        let step = step_of(size, self.spacing.get(draw));
        let overlap = overlap(size, step);

        RoundDraw {
            color: Vec4::from(self.color.into_components()),
            position: draw.position.q32_floor(),
            position_fract: draw.position.q32_fract(),
            softness: self.softness.get(draw),
            size,
            flow: flow_coeff(self.flow.get(draw), overlap),
            _pad: 0,
        }
    }

    fn step(&self, draw: Draw) -> f32 {
        step_of(self.size.get(draw), self.spacing.get(draw))
    }

    fn dirty(&self, draw: Self::Draw) -> Rectangle {
        Rectangle::new_half(draw.position, UVec2::splat((draw.size * 2.0).ceil() as u32))
    }

    fn replace_mode(&self) -> bool {
        self.erase
    }

    fn bridge_mode(&self) -> bool {
        false
    }

    fn set_pipeline(&self, cpass: &mut wgpu::ComputePass, pipeline: &LayerPipeline) {
        match self.erase {
            true => cpass.set_pipeline(&pipeline.brush_pipelines.round_erase),
            false => cpass.set_pipeline(&pipeline.brush_pipelines.round_over),
        }
    }
}

impl Element for RoundBrush {}
