use std::{fmt, ops};

use crate::measures::{Delta, Position};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct Rectangle {
    pub origin: Position,
    pub extend: Delta,
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

impl ops::Add<Delta> for Rectangle {
    type Output = Rectangle;
    fn add(self, rhs: Delta) -> Self::Output {
        Rectangle {
            origin: self.origin + rhs,
            extend: self.extend,
        }
    }
}

impl ops::AddAssign<Delta> for Rectangle {
    fn add_assign(&mut self, rhs: Delta) {
        self.origin += rhs;
    }
}

impl ops::Sub<Delta> for Rectangle {
    type Output = Rectangle;
    fn sub(self, rhs: Delta) -> Self::Output {
        Rectangle {
            origin: self.origin - rhs,
            extend: self.extend,
        }
    }
}

impl ops::SubAssign<Delta> for Rectangle {
    fn sub_assign(&mut self, rhs: Delta) {
        self.origin -= rhs;
    }
}

impl Rectangle {
    pub fn new(left: i32, down: i32, right: i32, up: i32) -> Rectangle {
        Rectangle {
            origin: Position::new(left, down),
            extend: Position::new(right, up) - Position::new(left, down),
        }
    }

    #[inline]
    pub fn width(self) -> u32 {
        self.extend.x as u32
    }

    #[inline]
    pub fn height(self) -> u32 {
        self.extend.y as u32
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
        self.origin.x + self.extend.x
    }

    #[inline]
    pub fn up(self) -> i32 {
        self.origin.y + self.extend.y
    }

    #[inline]
    pub fn left_down(self) -> Position {
        self.origin
    }

    #[inline]
    pub fn left_up(self) -> Position {
        self.origin + Delta::new(0, self.extend.y)
    }

    #[inline]
    pub fn right_down(self) -> Position {
        self.origin + Delta::new(self.extend.x, 0)
    }

    #[inline]
    pub fn right_up(self) -> Position {
        self.origin + self.extend
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

    pub fn contains(self, position: Position) -> bool {
        let normal = self.normalize();
        let delta = position.wrapping_sub(normal.origin);
        (delta.x as u32) < (normal.extend.x as u32) && (delta.y as u32) < (normal.extend.y as u32)
    }

    pub fn expand(self, val: i32) -> Rectangle {
        Rectangle {
            origin: self.origin - Delta::splat(val),
            extend: self.extend + Delta::splat(val * 2),
        }
    }

    pub fn normalize(self) -> Rectangle {
        let left = i32::min(self.origin.x, self.origin.x + self.extend.x);
        let down = i32::min(self.origin.y, self.origin.y + self.extend.y);
        let right = i32::max(self.origin.x, self.origin.x + self.extend.x);
        let up = i32::max(self.origin.y, self.origin.y + self.extend.y);
        Rectangle::new(left, down, right, up)
    }

    pub fn into_array(self) -> [i32; 4] {
        [self.left(), self.down(), self.right(), self.up()]
    }

    pub fn from_array(array: [i32; 4]) -> Rectangle {
        Rectangle {
            origin: Position {
                x: array[0],
                y: array[1],
            },
            extend: Delta {
                x: array[2] - array[0],
                y: array[3] - array[1],
            },
        }
    }
}
