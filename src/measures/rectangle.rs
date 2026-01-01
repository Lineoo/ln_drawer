use std::{fmt, ops};

use crate::measures::{Position, Size};

#[derive(Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rectangle {
    pub origin: Position,
    pub extend: Size,
}

impl fmt::Debug for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rectangle")
            .field("left", &self.left())
            .field("down", &self.down())
            .field("right", &self.right())
            .field("up", &self.up())
            .finish()
    }
}

impl fmt::Display for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entry(&self.left())
            .entry(&self.down())
            .entry(&self.right())
            .entry(&self.up())
            .finish()
    }
}

impl ops::Add<Position> for Rectangle {
    type Output = Rectangle;
    fn add(self, rhs: Position) -> Self::Output {
        Rectangle {
            origin: self.origin + rhs,
            extend: self.extend,
        }
    }
}

impl ops::Sub<Position> for Rectangle {
    type Output = Rectangle;
    fn sub(self, rhs: Position) -> Self::Output {
        Rectangle {
            origin: self.origin - rhs,
            extend: self.extend,
        }
    }
}

impl ops::AddAssign<Position> for Rectangle {
    fn add_assign(&mut self, rhs: Position) {
        self.origin += rhs;
    }
}

impl ops::SubAssign<Position> for Rectangle {
    fn sub_assign(&mut self, rhs: Position) {
        self.origin -= rhs;
    }
}

impl Rectangle {
    pub fn new(left: i32, down: i32, right: i32, up: i32) -> Rectangle {
        Rectangle {
            origin: Position::new(left.min(right), down.min(up)),
            extend: Size::new((right - left).unsigned_abs(), (up - down).unsigned_abs()),
        }
    }

    pub fn new_half(center: Position, half: Size) -> Rectangle {
        Rectangle {
            origin: center - Position::new(half.w as i32, half.h as i32),
            extend: half * 2,
        }
    }

    #[inline]
    pub fn width(self) -> u32 {
        self.extend.w
    }

    #[inline]
    pub fn height(self) -> u32 {
        self.extend.h
    }

    #[inline]
    pub fn left(self) -> i32 {
        self.origin.x
    }

    #[inline]
    pub fn down(self) -> i32 {
        self.origin.y
    }

    #[inline]
    pub fn right(self) -> i32 {
        self.origin.x.wrapping_add_unsigned(self.extend.w)
    }

    #[inline]
    pub fn up(self) -> i32 {
        self.origin.y.wrapping_add_unsigned(self.extend.h)
    }

    #[inline]
    pub fn left_down(self) -> Position {
        self.origin
    }

    #[inline]
    pub fn left_up(self) -> Position {
        Position::new(
            self.origin.x,
            self.origin.y.wrapping_add_unsigned(self.extend.h),
        )
    }

    #[inline]
    pub fn right_down(self) -> Position {
        Position::new(
            self.origin.x.wrapping_add_unsigned(self.extend.w),
            self.origin.y,
        )
    }

    #[inline]
    pub fn right_up(self) -> Position {
        Position::new(
            self.origin.x.wrapping_add_unsigned(self.extend.w),
            self.origin.y.wrapping_add_unsigned(self.extend.h),
        )
    }

    #[inline]
    pub fn with_left(self, left: i32) -> Rectangle {
        Rectangle::new(left, self.down(), self.right(), self.up())
    }

    #[inline]
    pub fn with_down(self, down: i32) -> Rectangle {
        Rectangle::new(self.left(), down, self.right(), self.up())
    }

    #[inline]
    pub fn with_right(self, right: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), right, self.up())
    }

    #[inline]
    pub fn with_up(self, up: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), self.right(), up)
    }

    #[inline]
    pub fn with_left_down(self, corner: Position) -> Rectangle {
        Rectangle::new(corner.x, corner.y, self.right(), self.up())
    }

    #[inline]
    pub fn with_left_up(self, corner: Position) -> Rectangle {
        Rectangle::new(corner.x, self.down(), self.right(), corner.y)
    }

    #[inline]
    pub fn with_right_down(self, corner: Position) -> Rectangle {
        Rectangle::new(self.left(), corner.y, corner.x, self.up())
    }

    #[inline]
    pub fn with_right_up(self, corner: Position) -> Rectangle {
        Rectangle::new(self.left(), self.down(), corner.x, corner.y)
    }

    #[inline]
    pub fn pad_left(self, left: i32, n: usize) -> Rectangle {
        Rectangle {
            origin: self.origin - Position::new(self.extend.w as i32 + left, 0) * n as i32,
            extend: self.extend,
        }
    }

    #[inline]
    pub fn pad_down(self, down: i32, n: usize) -> Rectangle {
        Rectangle {
            origin: self.origin - Position::new(0, self.extend.h as i32 + down) * n as i32,
            extend: self.extend,
        }
    }

    #[inline]
    pub fn pad_right(self, right: i32, n: usize) -> Rectangle {
        Rectangle {
            origin: self.origin + Position::new(self.extend.w as i32 + right, 0) * n as i32,
            extend: self.extend,
        }
    }

    #[inline]
    pub fn pad_up(self, up: i32, n: usize) -> Rectangle {
        Rectangle {
            origin: self.origin + Position::new(0, self.extend.h as i32 + up) * n as i32,
            extend: self.extend,
        }
    }

    pub fn expand(self, val: i32) -> Rectangle {
        Rectangle::new(
            self.origin.x.wrapping_sub(val),
            self.origin.y.wrapping_sub(val),
            (self.origin.x)
                .wrapping_add_unsigned(self.extend.w)
                .wrapping_add(val),
            (self.origin.y)
                .wrapping_add_unsigned(self.extend.h)
                .wrapping_add(val),
        )
    }

    /// will cause precise loss
    pub fn lerp(self, rhs: Rectangle, factor: f32) -> Rectangle {
        let x = self.origin.x as f32 * (1.0 - factor) + rhs.origin.x as f32 * factor;
        let y = self.origin.y as f32 * (1.0 - factor) + rhs.origin.y as f32 * factor;

        let w = self.extend.w as f32 * (1.0 - factor) + rhs.extend.w as f32 * factor;
        let h = self.extend.h as f32 * (1.0 - factor) + rhs.extend.h as f32 * factor;

        Rectangle {
            origin: Position {
                x: x.floor() as i32,
                y: y.floor() as i32,
            },
            extend: Size {
                w: w.floor() as u32,
                h: h.floor() as u32,
            },
        }
    }
}
