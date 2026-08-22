use std::{fmt, ops};

use glam::{IVec2, UVec2};

use crate::measures::Axis;

#[derive(Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rectangle {
    pub origin: IVec2,
    pub extend: UVec2,
}

impl fmt::Debug for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rectangle")
            .field("left", &self.left())
            .field("down", &self.down())
            .field("right", &self.right())
            .field("up", &self.up())
            .finish()
    }
}

impl fmt::Display for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entry(&self.left())
            .entry(&self.down())
            .entry(&self.right())
            .entry(&self.up())
            .finish()
    }
}

impl ops::Add<IVec2> for Rectangle {
    type Output = Rectangle;
    fn add(self, rhs: IVec2) -> Self::Output {
        Rectangle {
            origin: self.origin + rhs,
            extend: self.extend,
        }
    }
}

impl ops::Sub<IVec2> for Rectangle {
    type Output = Rectangle;
    fn sub(self, rhs: IVec2) -> Self::Output {
        Rectangle {
            origin: self.origin - rhs,
            extend: self.extend,
        }
    }
}

impl ops::AddAssign<IVec2> for Rectangle {
    fn add_assign(&mut self, rhs: IVec2) {
        self.origin += rhs;
    }
}

impl ops::SubAssign<IVec2> for Rectangle {
    fn sub_assign(&mut self, rhs: IVec2) {
        self.origin -= rhs;
    }
}

impl Rectangle {
    pub fn new(left: i32, down: i32, right: i32, up: i32) -> Rectangle {
        Rectangle {
            origin: IVec2::new(left.min(right), down.min(up)),
            extend: UVec2::new((right - left).unsigned_abs(), (up - down).unsigned_abs()),
        }
    }

    pub fn new_minmax(min: IVec2, max: IVec2) -> Rectangle {
        Rectangle {
            origin: min.min(max),
            extend: (min.max(max) - min.min(max)).abs().as_uvec2(),
        }
    }

    pub fn new_extend(left: i32, down: i32, width: u32, height: u32) -> Rectangle {
        Rectangle {
            origin: IVec2::new(left, down),
            extend: UVec2::new(width, height),
        }
    }

    pub fn new_half(center: IVec2, half: UVec2) -> Rectangle {
        Rectangle {
            origin: center - half.as_ivec2(),
            extend: half * 2,
        }
    }

    #[inline]
    pub const fn width(self) -> u32 {
        self.extend.x
    }

    #[inline]
    pub const fn height(self) -> u32 {
        self.extend.y
    }

    #[inline]
    pub const fn left(self) -> i32 {
        self.origin.x
    }

    #[inline]
    pub const fn down(self) -> i32 {
        self.origin.y
    }

    #[inline]
    pub const fn right(self) -> i32 {
        self.origin.x.wrapping_add_unsigned(self.extend.x)
    }

    #[inline]
    pub const fn up(self) -> i32 {
        self.origin.y.wrapping_add_unsigned(self.extend.y)
    }

    #[inline]
    pub const fn left_down(self) -> IVec2 {
        self.origin
    }

    #[inline]
    pub const fn left_up(self) -> IVec2 {
        IVec2::new(
            self.origin.x,
            self.origin.y.wrapping_add_unsigned(self.extend.x),
        )
    }

    #[inline]
    pub const fn right_down(self) -> IVec2 {
        IVec2::new(
            self.origin.x.wrapping_add_unsigned(self.extend.y),
            self.origin.y,
        )
    }

    #[inline]
    pub const fn right_up(self) -> IVec2 {
        IVec2::new(
            self.origin.x.wrapping_add_unsigned(self.extend.x),
            self.origin.y.wrapping_add_unsigned(self.extend.y),
        )
    }

    #[inline]
    pub fn with_left(self, left: i32) -> Rectangle {
        Rectangle::new(left, self.down(), self.right(), self.up())
    }

    #[inline]
    pub fn with_down(self, down: i32) -> Rectangle {
        Rectangle::new(self.left(), down, self.right(), self.up())
    }

    #[inline]
    pub fn with_right(self, right: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), right, self.up())
    }

    #[inline]
    pub fn with_up(self, up: i32) -> Rectangle {
        Rectangle::new(self.left(), self.down(), self.right(), up)
    }

    #[inline]
    pub fn with_left_down(self, corner: IVec2) -> Rectangle {
        Rectangle::new(corner.x, corner.y, self.right(), self.up())
    }

    #[inline]
    pub fn with_left_up(self, corner: IVec2) -> Rectangle {
        Rectangle::new(corner.x, self.down(), self.right(), corner.y)
    }

    #[inline]
    pub fn with_right_down(self, corner: IVec2) -> Rectangle {
        Rectangle::new(self.left(), corner.y, corner.x, self.up())
    }

    #[inline]
    pub fn with_right_up(self, corner: IVec2) -> Rectangle {
        Rectangle::new(self.left(), self.down(), corner.x, corner.y)
    }

    #[inline]
    pub const fn center(self) -> IVec2 {
        self.origin.wrapping_add(IVec2::new(
            self.extend.x as i32 / 2,
            self.extend.y as i32 / 2,
        ))
    }

    #[inline]
    pub const fn horizontal_center(self) -> i32 {
        self.origin.x.wrapping_add(self.extend.x as i32 / 2)
    }

    #[inline]
    pub const fn vertical_center(self) -> i32 {
        self.origin.y.wrapping_add(self.extend.y as i32 / 2)
    }

    pub fn expand(self, val: i32) -> Rectangle {
        Rectangle::new(
            self.origin.x.wrapping_sub(val),
            self.origin.y.wrapping_sub(val),
            (self.origin.x)
                .wrapping_add_unsigned(self.extend.x)
                .wrapping_add(val),
            (self.origin.y)
                .wrapping_add_unsigned(self.extend.y)
                .wrapping_add(val),
        )
    }

    pub fn grow(self, rhs: Rectangle) -> Rectangle {
        Rectangle::new(
            i32::min(self.left(), rhs.left()),
            i32::min(self.down(), rhs.down()),
            i32::max(self.right(), rhs.right()),
            i32::max(self.up(), rhs.up()),
        )
    }

    pub fn intersect(self, rhs: Rectangle) -> Option<Rectangle> {
        let left = self.left().max(rhs.left());
        let down = self.down().max(rhs.down());
        let right = self.right().min(rhs.right());
        let up = self.up().min(rhs.up());

        match right > left && up > down {
            true => Some(Rectangle::new(left, down, right, up)),
            false => None,
        }
    }

    pub fn contains(self, p: IVec2) -> bool {
        let delta = p.wrapping_sub(self.origin).as_uvec2();
        delta.x < self.extend.x && delta.y < self.extend.y
    }

    pub fn axis_new(start: i32, right: i32, end: i32, left: i32, axis: Axis) -> Rectangle {
        match axis {
            Axis::Right => Rectangle::new(start, right, end, left),
            Axis::Down => Rectangle::new(right, end, left, start),
            Axis::Left => Rectangle::new(end, left, start, right),
            Axis::Up => Rectangle::new(left, start, right, end),
        }
    }

    pub fn axis_start(self, axis: Axis) -> i32 {
        self.axis_end(axis.flip())
    }

    pub fn axis_end(self, axis: Axis) -> i32 {
        match axis {
            Axis::Left => self.left(),
            Axis::Down => self.down(),
            Axis::Right => self.right(),
            Axis::Up => self.up(),
        }
    }

    pub fn axis_length(self, axis: Axis) -> u32 {
        match axis.is_horizontal() {
            true => self.width(),
            false => self.height(),
        }
    }

    pub fn axis_center(self, axis: Axis) -> i32 {
        match axis.is_horizontal() {
            true => self.horizontal_center(),
            false => self.vertical_center(),
        }
    }
}
