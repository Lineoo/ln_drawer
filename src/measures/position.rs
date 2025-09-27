use std::{fmt, ops};

use crate::measures::{Rectangle, delta::Delta};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
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
impl ops::Add<Delta> for Position {
    type Output = Position;
    fn add(self, rhs: Delta) -> Self::Output {
        Position {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl ops::AddAssign<Delta> for Position {
    fn add_assign(&mut self, rhs: Delta) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl ops::Sub<Delta> for Position {
    type Output = Position;
    fn sub(self, rhs: Delta) -> Self::Output {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
impl ops::SubAssign<Delta> for Position {
    fn sub_assign(&mut self, rhs: Delta) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}
impl ops::Sub for Position {
    type Output = Delta;
    fn sub(self, rhs: Self) -> Self::Output {
        Delta {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
impl Position {
    pub fn new(x: i32, y: i32) -> Position {
        Position { x, y }
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
        Position::new(
            self.x.clamp(rect.origin.x, rect.origin.x + rect.extend.x),
            self.y.clamp(rect.origin.y, rect.origin.y + rect.extend.y),
        )
    }

    pub fn wrap(self, rect: Rectangle) -> Position {
        let delta = self - rect.origin;

        let w = (delta.x).rem_euclid(rect.width() as i32);
        let h = (delta.y).rem_euclid(rect.height() as i32);

        rect.origin + Delta::new(w, h)
    }
}
