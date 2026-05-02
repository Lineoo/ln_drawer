use palette::Srgba;

use crate::{measures::PositionFract, stroke::interpolate::Draw};

pub struct Modifier {
    pub min_size: f32,
    pub max_size: f32,
    pub size_force_exp: f32,
    pub min_flow: f32,
    pub max_flow: f32,
    pub flow_force_exp: f32,
    pub softness: f32,
    pub color: Srgba,
}

#[derive(Clone, Copy)]
pub struct DrawProcessed {
    pub color: Srgba,
    pub position: PositionFract,
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawProcessedGPU {
    pub color: [f32; 4],
    pub position: [i32; 4],
    pub softness: f32,
    pub size: f32,
    pub flow: f32,
    pub _pad: u32,
}

impl Modifier {
    pub fn process(&self, draw: Draw) -> DrawProcessed {
        DrawProcessed {
            position: draw.position,
            softness: self.softness,
            color: self.color,
            size: self.size(draw),
            flow: self.flow(draw),
        }
    }

    pub fn size(&self, draw: Draw) -> f32 {
        self.min_size + (self.max_size - self.min_size) * draw.force.powf(self.size_force_exp)
    }

    pub fn flow(&self, draw: Draw) -> f32 {
        self.min_flow + (self.max_flow - self.min_flow) * draw.force.powf(self.flow_force_exp)
    }
}
