use std::mem::size_of;

use glam::{I64Vec2, IVec2, UVec2, Vec4};
use ln_world::Element;

use crate::{
    brush_params,
    layer::{
        BrushKind, BrushStage, CompositeMode, DRAWS_ARRAY_CAPACITY, LayerPipeline, WriteMode,
        brush::{Brush, Draw, DrawsProcessor, param::BrushParam},
    },
    measures::{FI64Ext, IRect},
};

#[derive(Clone)]
pub struct PixelBrush {
    pub size: BrushParam<f32>,
    pub color: Vec4,
    pub erase: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PixelDraw {
    pub color: Vec4,
    pub position: IVec2,
    pub size: f32,
    pub _pad: u32,
}

impl Brush for PixelBrush {
    fn processor(&self, from: Option<Draw>, to: Draw, out: &mut Vec<u8>) -> (Draw, IRect, u32) {
        self.processor_output(from, to, out)
    }

    fn replace_mode(&self) -> bool {
        true
    }

    fn bridge_mode(&self) -> bool {
        false
    }

    fn set_pipeline(
        &self,
        cpass: &mut wgpu::ComputePass,
        pipeline: &LayerPipeline,
        write: WriteMode,
    ) {
        let composite = match self.erase {
            true => CompositeMode::Erase,
            false => CompositeMode::Replace,
        };
        cpass.set_pipeline(pipeline.brush_pipelines.get(
            BrushKind::Pixel,
            BrushStage::Draw,
            write,
            composite,
        ));
    }
}

impl DrawsProcessor for PixelBrush {
    type Draw = PixelDraw;

    fn process(&self, draw: Draw) -> Self::Draw {
        PixelDraw {
            color: self.color,
            position: draw.position.q32_floor(),
            size: self.size.get(draw),
            _pad: 0,
        }
    }

    fn step(&self, _draw: Draw) -> f32 {
        1.0
    }

    fn dirty(&self, draw: Self::Draw) -> IRect {
        let half = draw.size.max(0.0).floor() as u32 + 1;
        IRect::new_half(draw.position, UVec2::splat(half))
    }

    fn processor_output(
        &self,
        from: Option<Draw>,
        to: Draw,
        out: &mut Vec<u8>,
    ) -> (Draw, IRect, u32) {
        let Some(from) = from else {
            let processed = self.process(to);
            let dirty = self.dirty(processed);
            out.extend_from_slice(bytemuck::bytes_of(&processed));
            return (to, dirty, 1);
        };

        let limit = DRAWS_ARRAY_CAPACITY as usize - size_of::<Self::Draw>();

        let mut pixel = from.position.q32_floor();
        let target = to.position.q32_floor();

        let dx = (target.x - pixel.x).abs();
        let dy = (target.y - pixel.y).abs();
        let sx = (target.x - pixel.x).signum();
        let sy = (target.y - pixel.y).signum();
        let steps = dx.max(dy);
        let mut err = dx - dy;
        let mut index = 0;

        let mut cnt = 0;
        let mut curr = from;
        let mut dirty = IRect::new_half(from.position.q32_as_i32(), UVec2::ZERO);
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

            if out.len() >= limit {
                break;
            }

            index += 1;
            curr.position = I64Vec2::q32_from_i32(pixel);
            let progress = match steps {
                0 => 1.0,
                _ => index as f32 / steps as f32,
            };
            curr.force = from.force + (to.force - from.force) * progress;

            let processed = self.process(curr);
            dirty = dirty.grow(self.dirty(processed));
            out.extend_from_slice(bytemuck::bytes_of(&processed));
            cnt += 1;
        }

        (curr, dirty, cnt)
    }
}

brush_params!(PixelBrush, {
    Size => Scalar(size),
    Color => Color(color),
    Erase => Toggle(erase),
});

impl Element for PixelBrush {}
