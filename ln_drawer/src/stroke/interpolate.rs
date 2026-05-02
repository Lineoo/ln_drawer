use crate::measures::{Fract, PositionFract};

const MAX_DRAWS: u64 = 500;

pub struct Interpolation {
    pub step: fn(Draw) -> f32,
}

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: PositionFract,
    pub force: f32,
}

impl Interpolation {
    pub fn interpolate(&self, prev: Draw, next: Draw, buf: &mut Vec<Draw>) {
        buf.clear();

        let mut curr = prev;
        let whole_dist = prev.position.distance(next.position).into_f32();
        while curr.position.distance(next.position).into_f32() >= (self.step)(curr)
            && buf.len() < MAX_DRAWS as usize
        {
            let step = Fract::from_f32((self.step)(curr));
            curr.position = curr.position.move_towards(next.position, step);
            let curr_dist = curr.position.distance(next.position).into_f32();
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - curr_dist / whole_dist,
            };
            curr.force = (1.0 - progress) * prev.force + progress * next.force;
            buf.push(curr);
        }
    }
}
