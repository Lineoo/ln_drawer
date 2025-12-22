use std::{fmt, ops};

use crate::measures::Position;

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl fmt::Debug for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Size").field(&self.w).field(&self.h).finish()
    }
}

impl ops::Add for Size {
    type Output = Size;
    fn add(self, rhs: Self) -> Self::Output {
        Size {
            w: self.w + rhs.w,
            h: self.h + rhs.h,
        }
    }
}

impl ops::Sub for Size {
    type Output = Size;
    fn sub(self, rhs: Self) -> Self::Output {
        Size {
            w: self.w - rhs.w,
            h: self.h - rhs.h,
        }
    }
}

impl ops::AddAssign for Size {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::SubAssign for Size {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Size {
    pub const MAX: Size = Size {
        w: u32::MAX,
        h: u32::MAX,
    };

    pub const fn new(w: u32, h: u32) -> Size {
        Size { w, h }
    }

    pub const fn splat(n: u32) -> Size {
        Size { w: n, h: n }
    }

    pub fn into_array(self) -> [u32; 2] {
        [self.w, self.h]
    }
}
