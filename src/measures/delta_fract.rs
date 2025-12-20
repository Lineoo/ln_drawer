use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct DeltaFract {
    pub x: i32,
    pub xf: u32,
    pub y: i32,
    pub yf: u32,
}

impl fmt::Debug for DeltaFract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DeltaFract")
            .field(&self.x)
            .field(&self.xf)
            .field(&self.y)
            .field(&self.yf)
            .finish()
    }
}

impl ops::Add for DeltaFract {
    type Output = DeltaFract;
    fn add(self, rhs: Self) -> Self::Output {
        DeltaFract {
            x: self.x.wrapping_add(rhs.x),
            xf: self.xf.wrapping_add(rhs.xf),
            y: self.y.wrapping_add(rhs.y),
            yf: self.yf.wrapping_add(rhs.yf),
        }
    }
}

impl ops::AddAssign for DeltaFract {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::Sub for DeltaFract {
    type Output = DeltaFract;
    fn sub(self, rhs: Self) -> Self::Output {
        DeltaFract {
            x: self.x.wrapping_sub(rhs.x),
            xf: self.xf.wrapping_sub(rhs.xf),
            y: self.y.wrapping_sub(rhs.y),
            yf: self.yf.wrapping_sub(rhs.yf),
        }
    }
}

impl ops::SubAssign for DeltaFract {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl DeltaFract {
    pub fn new(x: i32, xf: u32, y: i32, yf: u32) -> DeltaFract {
        DeltaFract { x, xf, y, yf }
    }

    pub fn splat(n: i32, nf: u32) -> DeltaFract {
        DeltaFract {
            x: n,
            xf: nf,
            y: n,
            yf: nf,
        }
    }

    pub fn into_array(self) -> ([i32; 2], [u32; 2]) {
        ([self.x, self.y], [self.xf, self.yf])
    }
}
