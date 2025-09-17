use std::{fmt, ops};

use crate::measures::{delta::Delta, position::Position};

#[derive(Default, Clone, Copy)]
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
    #[inline]
    pub fn width(self) -> u32 {
        self.extend.x as u32
    }

    #[inline]
    pub fn height(self) -> u32 {
        self.extend.x as u32
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
