use crate::{
    measures::{Position, Rectangle, Size},
    stroke::modifier::DrawProcessed,
};

pub struct Dirty {
    pub bounding: fn(DrawProcessed) -> Rectangle,
}

impl Dirty {
    pub fn compute(&self, start: Position, buf: &[DrawProcessed]) -> Rectangle {
        let mut dirty = Rectangle::new_half(start, Size::splat(0));

        for draw in buf {
            dirty = dirty.grow((self.bounding)(*draw));
        }

        dirty
    }
}
