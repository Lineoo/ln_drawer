use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fract {
    pub n: i32,
    pub nf: u32,
}

impl fmt::Debug for Fract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Fract")
            .field(&self.n)
            .field(&self.nf)
            .finish()
    }
}

impl ops::Add for Fract {
    type Output = Fract;
    fn add(self, rhs: Self) -> Self::Output {
        let (nf, on) = self.nf.overflowing_add(rhs.nf);
        let mut n = self.n.wrapping_add(rhs.n);

        if on {
            n += 1;
        }

        Fract { n, nf }
    }
}

impl ops::Add<i32> for Fract {
    type Output = Fract;
    fn add(self, rhs: i32) -> Self::Output {
        Fract {
            n: self.n + rhs,
            nf: self.nf,
        }
    }
}

impl ops::AddAssign for Fract {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::AddAssign<i32> for Fract {
    fn add_assign(&mut self, rhs: i32) {
        self.n += rhs;
    }
}

impl ops::Sub for Fract {
    type Output = Fract;
    fn sub(self, rhs: Self) -> Self::Output {
        let (nf, on) = self.nf.overflowing_sub(rhs.nf);
        let mut n = self.n.wrapping_sub(rhs.n);

        if on {
            n -= 1;
        }

        Fract { n, nf }
    }
}

impl ops::Sub<i32> for Fract {
    type Output = Fract;
    fn sub(self, rhs: i32) -> Self::Output {
        Fract {
            n: self.n - rhs,
            nf: self.nf,
        }
    }
}

impl ops::SubAssign for Fract {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl ops::SubAssign<i32> for Fract {
    fn sub_assign(&mut self, rhs: i32) {
        self.n -= rhs;
    }
}

impl Fract {
    pub fn new(n: i32, nf: u32) -> Fract {
        Fract { n, nf }
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
}
