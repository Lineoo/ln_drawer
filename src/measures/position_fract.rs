use std::{fmt, ops};

use crate::measures::{Fract, Position};

#[derive(Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PositionFract {
    pub x: Fract,
    pub y: Fract,
}

impl fmt::Debug for PositionFract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PositionFract")
            .field(&self.x)
            .field(&self.y)
            .finish()
    }
}

impl fmt::Display for PositionFract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.x).field(&self.y).finish()
    }
}

impl ops::Add for PositionFract {
    type Output = PositionFract;
    fn add(self, rhs: PositionFract) -> Self::Output {
        PositionFract {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl ops::Sub for PositionFract {
    type Output = PositionFract;
    fn sub(self, rhs: Self) -> Self::Output {
        PositionFract {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl ops::Mul<Fract> for PositionFract {
    type Output = PositionFract;
    fn mul(self, rhs: Fract) -> Self::Output {
        PositionFract {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl ops::AddAssign for PositionFract {
    fn add_assign(&mut self, rhs: PositionFract) {
        *self = *self + rhs;
    }
}

impl ops::SubAssign for PositionFract {
    fn sub_assign(&mut self, rhs: PositionFract) {
        *self = *self - rhs;
    }
}

impl ops::MulAssign<Fract> for PositionFract {
    fn mul_assign(&mut self, rhs: Fract) {
        *self = *self * rhs;
    }
}

impl PositionFract {
    pub const ZERO: Self = PositionFract {
        x: Fract::ZERO,
        y: Fract::ZERO,
    };

    pub const fn new(x: Fract, y: Fract) -> PositionFract {
        PositionFract { x, y }
    }

    pub const fn splat(n: Fract) -> PositionFract {
        PositionFract { x: n, y: n }
    }

    pub fn floor(self) -> Position {
        Position {
            x: self.x.floor(),
            y: self.y.floor(),
        }
    }

    pub fn round(self) -> Position {
        Position {
            x: self.x.round(),
            y: self.y.round(),
        }
    }

    pub fn length(self) -> Fract {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn distance(self, rhs: PositionFract) -> Fract {
        (self - rhs).length()
    }

    pub fn normalize(self) -> PositionFract {
        self * self.length().recip()
    }

    pub fn move_towards(self, dst: PositionFract, distance: Fract) -> PositionFract {
        let delta = dst - self;
        if delta.length() >= distance {
            self + delta.normalize() * distance
        } else {
            dst
        }
    }

    pub fn into_array(self) -> [i32; 2] {
        [self.x.n, self.y.n]
    }

    pub fn into_arrayf(self) -> [u32; 2] {
        [self.x.nf, self.y.nf]
    }

    pub fn from_array(array: [i32; 2], arrayf: [u32; 2]) -> PositionFract {
        PositionFract {
            x: Fract::new(array[0], arrayf[0]),
            y: Fract::new(array[1], arrayf[1]),
        }
    }
}
