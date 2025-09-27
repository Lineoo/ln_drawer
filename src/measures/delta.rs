use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Delta {
    pub w: i32,
    pub h: i32,
}
impl fmt::Debug for Delta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Delta")
            .field(&self.w)
            .field(&self.h)
            .finish()
    }
}
impl ops::Add for Delta {
    type Output = Delta;
    fn add(self, rhs: Self) -> Self::Output {
        Delta {
            w: self.w + rhs.w,
            h: self.h + rhs.h,
        }
    }
}
impl ops::AddAssign for Delta {
    fn add_assign(&mut self, rhs: Self) {
        self.w += rhs.w;
        self.h += rhs.h;
    }
}
impl ops::Sub for Delta {
    type Output = Delta;
    fn sub(self, rhs: Self) -> Self::Output {
        Delta {
            w: self.w - rhs.w,
            h: self.h - rhs.h,
        }
    }
}
impl ops::SubAssign for Delta {
    fn sub_assign(&mut self, rhs: Self) {
        self.w -= rhs.w;
        self.h -= rhs.h;
    }
}
impl Delta {
    pub fn new(w: i32, h: i32) -> Delta {
        Delta { w, h }
    }

    pub fn splat(n: i32) -> Delta {
        Delta { w: n, h: n }
    }
}
