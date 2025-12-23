use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct Fract {
    pub n: i32,
    pub nf: u32,
}

impl fmt::Debug for Fract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Fract").field(&self.into_f64()).finish()
    }
}

impl ops::Neg for Fract {
    type Output = Fract;
    fn neg(self) -> Self::Output {
        Fract {
            n: -self.n,
            nf: self.nf.reverse_bits().wrapping_add(1),
        }
    }
}

impl ops::Add for Fract {
    type Output = Fract;
    fn add(self, rhs: Self) -> Self::Output {
        let (nf, on) = self.nf.overflowing_add(rhs.nf);
        let mut n = self.n.wrapping_add(rhs.n);

        if on {
            n = n.wrapping_add(1);
        }

        Fract { n, nf }
    }
}

impl ops::Sub for Fract {
    type Output = Fract;
    fn sub(self, rhs: Self) -> Self::Output {
        let (nf, on) = self.nf.overflowing_sub(rhs.nf);
        let mut n = self.n.wrapping_sub(rhs.n);

        if on {
            n = n.wrapping_sub(1);
        }

        Fract { n, nf }
    }
}

impl ops::Mul for Fract {
    type Output = Fract;
    fn mul(self, rhs: Fract) -> Self::Output {
        Fract::from_f64(self.into_f64() * rhs.into_f64())
    }
}

impl ops::AddAssign for Fract {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::SubAssign for Fract {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl ops::MulAssign for Fract {
    fn mul_assign(&mut self, rhs: Fract) {
        *self = *self * rhs;
    }
}

impl Fract {
    pub const ONE: Fract = Fract { n: 1, nf: 0 };

    pub const fn new(n: i32, nf: u32) -> Fract {
        Fract { n, nf }
    }

    pub fn floor(self) -> i32 {
        self.n
    }

    pub fn exp2(self) -> Fract {
        Fract::from_f64(self.into_f64().exp2())
    }

    pub fn from_f32(f: f32) -> Fract {
        Fract {
            n: f.floor() as i32,
            nf: ((f - f.floor()) * 32f32.exp2()) as u32,
        }
    }

    pub fn from_f64(f: f64) -> Fract {
        Fract {
            n: f.floor() as i32,
            nf: ((f - f.floor()) * 32f64.exp2()) as u32,
        }
    }

    pub fn into_f32(self) -> f32 {
        self.n as f32 + self.nf as f32 * (-32f32).exp2()
    }

    pub fn into_f64(self) -> f64 {
        self.n as f64 + self.nf as f64 * (-32f64).exp2()
    }
}
