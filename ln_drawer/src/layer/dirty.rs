use glam::{IVec2, UVec2};

use crate::{measures::Rectangle, layer::modifier::DrawProcessed};

pub struct Dirty {
    pub bounding: fn(DrawProcessed) -> Rectangle,
}

impl Dirty {
    pub fn compute(&self, start: IVec2, buf: &[DrawProcessed]) -> Rectangle {
        let mut dirty = Rectangle::new_half(start, UVec2::splat(0));

        for draw in buf {
            dirty = dirty.grow((self.bounding)(*draw));
        }

        dirty
    }
}
