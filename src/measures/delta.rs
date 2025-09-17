use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Delta {
    pub x: i32,
    pub y: i32,
}
impl fmt::Debug for Delta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Delta")
            .field(&self.x)
            .field(&self.y)
            .finish()
    }
}
impl ops::Add for Delta {
    type Output = Delta;
    fn add(self, rhs: Self) -> Self::Output {
        Delta {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl ops::AddAssign for Delta {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}
impl ops::Sub for Delta {
    type Output = Delta;
    fn sub(self, rhs: Self) -> Self::Output {
        Delta {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
impl ops::SubAssign for Delta {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}
