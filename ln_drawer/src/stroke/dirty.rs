use crate::{
    measures::{Rectangle, Size},
    stroke::{CHUNK_SIZE, modifier::DrawProcessed},
};

pub struct Dirty {
    pub bounding: fn(DrawProcessed) -> Rectangle,
}

pub struct DrawDirty {
    pub dirty: Rectangle,
    pub chunk_src: (i32, i32),
    pub chunk_dst: (i32, i32),
}

impl Dirty {
    pub fn compute(&self, buf: &[DrawProcessed]) -> DrawDirty {
        let mut dirty = Rectangle::new_half(buf[0].position.round(), Size::splat(0));

        for draw in buf {
            // dirty = dirty.grow(Rectangle::new_half(
            //     draw.position.round(),
            //     Size::splat((draw.size * 2.0).ceil() as u32),
            // ));
            dirty = dirty.grow((self.bounding)(*draw));
        }

        let chunk_src = (
            dirty.left().div_euclid(CHUNK_SIZE as i32),
            dirty.down().div_euclid(CHUNK_SIZE as i32),
        );

        let chunk_dst = (
            (dirty.right() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
            (dirty.up() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
        );

        DrawDirty {
            dirty,
            chunk_src,
            chunk_dst,
        }
    }
}
