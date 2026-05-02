use crate::{
    measures::{Fract, PositionFract},
    stroke::modifier::{DrawProcessed, Modifier},
};

pub struct Interpolation {
    pub step: fn(DrawProcessed) -> f32,
}

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: PositionFract,
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
        let whole_dist = prev.position.distance(next.position).into_f32();
        while curr_draw.position.distance(next.position).into_f32() >= (self.step)(curr_proc)
            && buf.len() < super::MAX_STROKE as usize
        {
            let step = Fract::from_f32((self.step)(curr_proc));
            curr_draw.position = curr_draw.position.move_towards(next.position, step);
            let curr_dist = curr_draw.position.distance(next.position).into_f32();
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - curr_dist / whole_dist,
            };
            curr_draw.force = (1.0 - progress) * prev.force + progress * next.force;
            curr_proc = modifier.process(curr_draw);
            buf.push(curr_proc);
        }

        curr_draw
    }
}
