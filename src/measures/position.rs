use std::{fmt, ops};

use crate::measures::{Rectangle, Size};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Position")
            .field(&self.x)
            .field(&self.y)
            .finish()
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.x).field(&self.y).finish()
    }
}

impl ops::Add for Position {
    type Output = Position;
    fn add(self, rhs: Position) -> Self::Output {
        Position {
            x: self.x.wrapping_add(rhs.x),
            y: self.y.wrapping_add(rhs.y),
        }
    }
}

impl ops::Sub for Position {
    type Output = Position;
    fn sub(self, rhs: Self) -> Self::Output {
        Position {
            x: self.x.wrapping_sub(rhs.x),
            y: self.y.wrapping_sub(rhs.y),
        }
    }
}

impl ops::AddAssign for Position {
    fn add_assign(&mut self, rhs: Position) {
        *self = *self + rhs
    }
}

impl ops::SubAssign for Position {
    fn sub_assign(&mut self, rhs: Position) {
        *self = *self - rhs
    }
}

impl Position {
    pub const MIN: Position = Position {
        x: i32::MIN,
        y: i32::MIN,
    };

    pub const fn new(x: i32, y: i32) -> Position {
        Position { x, y }
    }

    pub const fn splat(n: i32) -> Position {
        Position { x: n, y: n }
    }

    pub fn into_array(self) -> [i32; 2] {
        [self.x, self.y]
    }

    pub fn from_array(array: [i32; 2]) -> Position {
        Position {
            x: array[0],
            y: array[1],
        }
    }

    pub fn clamp(self, rect: Rectangle) -> Position {
        Position {
            x: self.x.clamp(rect.left(), rect.right()),
            y: self.y.clamp(rect.down(), rect.up()),
        }
    }

    pub fn within(self, rect: Rectangle) -> bool {
        let delta = self.wrapping_sub(rect.origin);
        delta.w < rect.extend.w && delta.h < rect.extend.h
    }

    pub fn wrapping_add(self, rhs: Size) -> Self {
        Position {
            x: self.x.wrapping_add_unsigned(rhs.w),
            y: self.y.wrapping_add_unsigned(rhs.h),
        }
    }

    pub fn wrapping_sub(self, rhs: Self) -> Size {
        Size {
            w: self.x.wrapping_sub(rhs.x).cast_unsigned(),
            h: self.y.wrapping_sub(rhs.y).cast_unsigned(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::measures::{Position, Rectangle};

    #[test]
    fn clamp() {
        let rect = Rectangle::new(-103, -100, 25, 76);
        assert_eq!(Position::new(-256, 2).clamp(rect), Position::new(-103, 2));
    }
}
