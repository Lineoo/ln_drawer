use std::{fmt, ops};

use crate::measures::{DeltaFract, Position};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct PositionFract {
    pub x: i32,
    pub xf: u32,
    pub y: i32,
    pub yf: u32,
}

impl fmt::Debug for PositionFract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PositionFract")
            .field(&self.x)
            .field(&self.xf)
            .field(&self.y)
            .field(&self.yf)
            .finish()
    }
}

impl fmt::Display for PositionFract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.x).field(&self.y).finish()
    }
}

impl ops::Add<DeltaFract> for PositionFract {
    type Output = PositionFract;
    fn add(self, rhs: DeltaFract) -> Self::Output {
        let (xf, ox) = self.xf.overflowing_add(rhs.xf);
        let (yf, oy) = self.yf.overflowing_add(rhs.yf);
        let mut x = self.x.wrapping_add(rhs.x);
        let mut y = self.y.wrapping_add(rhs.y);

        if ox {
            x = x.wrapping_add(1);
        }

        if oy {
            y = y.wrapping_add(1);
        }

        PositionFract { x, xf, y, yf }
    }
}

impl ops::AddAssign<DeltaFract> for PositionFract {
    fn add_assign(&mut self, rhs: DeltaFract) {
        *self = *self + rhs;
    }
}

impl ops::Sub<DeltaFract> for PositionFract {
    type Output = PositionFract;
    fn sub(self, rhs: DeltaFract) -> Self::Output {
        let (xf, ox) = self.xf.overflowing_sub(rhs.xf);
        let (yf, oy) = self.yf.overflowing_sub(rhs.yf);
        let mut x = self.x.wrapping_sub(rhs.x);
        let mut y = self.y.wrapping_sub(rhs.y);

        if ox {
            x -= 1;
        }

        if oy {
            y -= 1;
        }

        PositionFract { x, xf, y, yf }
    }
}

impl ops::SubAssign<DeltaFract> for PositionFract {
    fn sub_assign(&mut self, rhs: DeltaFract) {
        *self = *self - rhs;
    }
}

impl ops::Sub for PositionFract {
    type Output = DeltaFract;
    fn sub(self, rhs: Self) -> Self::Output {
        let (xf, ox) = self.xf.overflowing_sub(rhs.xf);
        let (yf, oy) = self.yf.overflowing_sub(rhs.yf);
        let mut x = self.x.wrapping_sub(rhs.x);
        let mut y = self.y.wrapping_sub(rhs.y);

        if ox {
            x = x.wrapping_sub(1);
        }

        if oy {
            y = y.wrapping_sub(1);
        }

        DeltaFract { x, xf, y, yf }
    }
}

impl PositionFract {
    pub fn new(x: i32, xf: u32, y: i32, yf: u32) -> PositionFract {
        PositionFract { x, xf, y, yf }
    }

    pub fn splat(n: i32, nf: u32) -> PositionFract {
        PositionFract {
            x: n,
            xf: nf,
            y: n,
            yf: nf,
        }
    }

    pub fn floor(self) -> Position {
        Position {
            x: self.x,
            y: self.y,
        }
    }

    pub fn into_array(self) -> [i32; 2] {
        [self.x, self.y]
    }

    pub fn into_arrayf(self) -> [u32; 2] {
        [self.xf, self.yf]
    }

    pub fn from_array(array: [i32; 2], arrayf: [u32; 2]) -> PositionFract {
        PositionFract {
            x: array[0],
            xf: arrayf[0],
            y: array[1],
            yf: arrayf[1],
        }
    }
}

mod test {
    use crate::measures::{DeltaFract, PositionFract};

    #[test]
    fn sub() {
        let a = PositionFract::new(12, 0, 14, 0);
        let b = PositionFract::new(1, 0, 1, 0);
        assert_eq!(a - b, DeltaFract::new(11, 0, 13, 0));
    }
}