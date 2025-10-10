use std::{fmt, ops};

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZOrder {
    pub idx: isize,
}
impl fmt::Debug for ZOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ZOrder").field(&self.idx).finish()
    }
}
impl ops::Add for ZOrder {
    type Output = ZOrder;
    fn add(self, rhs: Self) -> Self::Output {
        ZOrder {
            idx: self.idx + rhs.idx,
        }
    }
}
impl ops::AddAssign for ZOrder {
    fn add_assign(&mut self, rhs: Self) {
        self.idx += rhs.idx;
    }
}
impl ops::Sub for ZOrder {
    type Output = ZOrder;
    fn sub(self, rhs: Self) -> Self::Output {
        ZOrder {
            idx: self.idx - rhs.idx,
        }
    }
}
impl ops::SubAssign for ZOrder {
    fn sub_assign(&mut self, rhs: Self) {
        self.idx -= rhs.idx;
    }
}
impl ZOrder {
    pub fn new(idx: isize) -> ZOrder {
        ZOrder { idx }
    }
}
