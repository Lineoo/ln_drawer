use std::mem::size_of;

use glam::{I64Vec2, IVec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::{
        DRAWS_ARRAY_CAPACITY, LayerPipeline,
        brush::{Brush, Draw, param::BrushParam},
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct PixelBrush {
    pub size: BrushParam<f32>,
    pub flow: BrushParam<f32>,
    pub color: Srgba,
    pub erase: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PixelDraw {
    pub color: Vec4,
    pub position: IVec2,
    pub size: f32,
    pub flow: f32,
}

impl Brush for PixelBrush {
    type Draw = PixelDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        PixelDraw {
            color: Vec4::from(self.color.into_components()),
            position: draw.position.q32_floor(),
            size: self.size.get(draw),
            flow: self.flow.get(draw),
        }
    }

    fn step(&self, _draw: Self::Draw) -> f32 {
        1.0
    }

    fn interpolate(&self, from: Draw, to: Draw, out: &mut Vec<Self::Draw>) -> Draw {
        let cap = DRAWS_ARRAY_CAPACITY as usize / size_of::<Self::Draw>();

        let mut pixel = from.position.q32_floor();
        let target = to.position.q32_floor();

        let dx = (target.x - pixel.x).abs();
        let dy = (target.y - pixel.y).abs();
        let sx = (target.x - pixel.x).signum();
        let sy = (target.y - pixel.y).signum();
        let steps = dx.max(dy);
        let mut err = dx - dy;
        let mut index = 0;

        let mut curr = from;
        while pixel != target {
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                pixel.x += sx;
            }
            if e2 < dx {
                err += dx;
                pixel.y += sy;
            }

            if out.len() >= cap {
                return curr;
            }

            index += 1;
            curr.position = I64Vec2::q32_from_i32(pixel);
            let progress = match steps {
                0 => 1.0,
                _ => index as f32 / steps as f32,
            };
            curr.force = from.force + (to.force - from.force) * progress;
            out.push(self.process(curr));
        }

        to
    }

    fn dirty(&self, draw: Self::Draw) -> Rectangle {
        let half = draw.size.max(0.0).floor() as u32 + 1;
        Rectangle::new_half(draw.position, UVec2::splat(half))
    }

    fn replace_mode(&self) -> bool {
        self.erase
    }

    fn bridge_mode(&self) -> bool {
        false
    }

    fn set_pipeline(&self, cpass: &mut wgpu::ComputePass, pipeline: &LayerPipeline) {
        match self.erase {
            true => cpass.set_pipeline(&pipeline.brush_pipelines.pixel_erase),
            false => cpass.set_pipeline(&pipeline.brush_pipelines.pixel_over),
        }
    }
}

impl Element for PixelBrush {}
