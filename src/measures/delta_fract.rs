use std::{fmt, ops};

use glam::Vec2;

use crate::measures::Fract;

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

impl ops::Mul<f32> for DeltaFract {
    type Output = DeltaFract;
    fn mul(self, rhs: f32) -> Self::Output {
        let rx = (self.x as f32 + self.xf as f32 * (-32f32).exp2()) * rhs;
        let ry = (self.y as f32 + self.yf as f32 * (-32f32).exp2()) * rhs;
        DeltaFract {
            x: rx.floor() as i32,
            xf: ((rx - rx.floor()) * 32f32.exp2()) as u32,
            y: ry.floor() as i32,
            yf: ((ry - ry.floor()) * 32f32.exp2()) as u32,
        }
    }
}

impl ops::MulAssign<f32> for DeltaFract {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

impl ops::Mul<Fract> for DeltaFract {
    type Output = DeltaFract;
    fn mul(self, rhs: Fract) -> Self::Output {
        let (xf, ox) = self.xf.carrying_mul(rhs.nf, 0);
        let x = (self.x.unsigned_abs().saturating_mul(rhs.nf) + ox) as i32 * self.x.signum();
        let (yf, oy) = self.yf.carrying_mul(rhs.nf, 0);
        let y = (self.y.unsigned_abs().saturating_mul(rhs.nf) + oy) as i32 * self.y.signum();
        DeltaFract { x, xf, y, yf }
    }
}

impl ops::MulAssign<Fract> for DeltaFract {
    fn mul_assign(&mut self, rhs: Fract) {
        *self = *self * rhs;
    }
}

impl ops::Div<f32> for DeltaFract {
    type Output = DeltaFract;
    fn div(self, rhs: f32) -> Self::Output {
        let rx = (self.x as f32 + self.xf as f32 * (-32f32).exp2()) / rhs;
        let ry = (self.y as f32 + self.yf as f32 * (-32f32).exp2()) / rhs;
        DeltaFract {
            x: rx.floor() as i32,
            xf: ((rx - rx.floor()) * 32f32.exp2()) as u32,
            y: ry.floor() as i32,
            yf: ((ry - ry.floor()) * 32f32.exp2()) as u32,
        }
    }
}

impl ops::DivAssign<f32> for DeltaFract {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
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

#[cfg(test)]
mod test {
    use crate::measures::DeltaFract;

    #[test]
    fn mul() {
        let a = DeltaFract::new(1, 0, 1, 0);
        assert_eq!(a * 2.0, DeltaFract::new(2, 0, 2, 0));
    }
}