use glam::I64Vec2;

use crate::{
    measures::FI64Ext,
    stroke::modifier::{DrawProcessed, Modifier},
};

pub struct Interpolation {
    pub step: fn(DrawProcessed) -> f32,
}

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: I64Vec2,
    pub force: f32,
}

impl Interpolation {
    pub fn interpolate(
        &self,
        prev: Option<Draw>,
        next: Draw,
        modifier: &Modifier,
        buf: &mut Vec<DrawProcessed>,
    ) -> Draw {
        buf.clear();

        let prev = prev.unwrap_or_else(|| {
            buf.push(modifier.process(next));
            next
        });

        let mut curr_draw = prev;
        let mut curr_proc = modifier.process(curr_draw);
        let whole_dist = prev
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64());
        while curr_draw
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64())
            >= (self.step)(curr_proc) as f64
            && buf.len() < super::MAX_STROKE as usize
        {
            let step = (self.step)(curr_proc);
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
            curr_proc = modifier.process(curr_draw);
            buf.push(curr_proc);
        }

        curr_draw
    }
}
