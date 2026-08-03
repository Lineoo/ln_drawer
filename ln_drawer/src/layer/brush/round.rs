use glam::{I64Vec2, UVec2, Vec4};
use ln_world::Element;
use palette::Srgba;

use crate::{
    layer::brush::Draw,
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone, Copy)]
pub struct RoundBrush {
    pub min_size: f32,
    pub max_size: f32,
    pub size_force_exp: f32,
    pub min_flow: f32,
    pub max_flow: f32,
    pub flow_force_exp: f32,
    pub softness: f32,
    pub color: Srgba,
    pub erase: bool,
}

impl RoundBrush {
    pub fn process(&self, draw: Draw) -> RoundDraw {
        RoundDraw {
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

    pub fn step(&self, draw: RoundDraw) -> f32 {
        draw.size / 5.0
    }

    pub fn interpolate(&self, prev: Option<Draw>, next: Draw, buf: &mut Vec<RoundDraw>) -> Draw {
        buf.clear();

        let prev = prev.unwrap_or_else(|| {
            buf.push(self.process(next));
            next
        });

        let mut curr_draw = prev;
        let mut curr_proc = self.process(curr_draw);
        let whole_dist = prev
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64());
        while curr_draw
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64())
            >= self.step(curr_proc) as f64
            && buf.len() < super::MAX_STROKE as usize
        {
            let step = self.step(curr_proc);
            curr_draw.position = I64Vec2::q32_from_f64(
                curr_draw
                    .position
                    .q32_as_f64()
                    .move_towards(next.position.q32_as_f64(), step as f64),
            );
            let curr_dist = curr_draw
                .position
                .q32_as_f64()
                .distance(next.position.q32_as_f64());
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - (curr_dist / whole_dist) as f32,
            };
            curr_draw.force = (1.0 - progress) * prev.force + progress * next.force;
            curr_proc = self.process(curr_draw);
            buf.push(curr_proc);
        }

        curr_draw
    }

    /// - Normal Mode: 
    ///     - Scratch chunks start with __transparent texture__.
    ///     - Render in __over__ mode
    ///     - Merge in __over__ mode
    /// - Replace Mode:
    ///     - Scratch chunks start with __data from destination layer__.
    ///     - Render in __replace__ mode
    ///     - Merge in __replace__ mode
    pub fn replace_mode(&self) -> bool {
        self.erase
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
