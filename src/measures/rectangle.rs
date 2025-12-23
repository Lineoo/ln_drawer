use std::{fmt, ops};

use crate::measures::{Position, Size};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
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
    pub fn with_up(self, up: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), self.right(), up)
    }

    #[inline]
    pub fn with_right(self, right: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), right, self.up())
    }

    #[inline]
    pub fn with_down(self, down: i32) -> Rectangle {
        Rectangle::new(self.left(), down, self.right(), self.up())
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
}
