use glam::{IVec2, UVec2};

#[derive(Debug, Clone, Copy)]
pub enum Axis {
    Left,
    Down,
    Right,
    Up,
}

impl Axis {
    /// Rotate clockwise. Use [`rotate_rev`](Axis::rotate_rev) for counter-clockwise.
    pub fn rotate(self) -> Axis {
        match self {
            Axis::Left => Axis::Up,
            Axis::Up => Axis::Right,
            Axis::Right => Axis::Down,
            Axis::Down => Axis::Left,
        }
    }

    /// Rotate counter-clockwise. Use [`rotate`](Axis::rotate) for clockwise.
    pub fn rotate_rev(self) -> Axis {
        match self {
            Axis::Left => Axis::Down,
            Axis::Down => Axis::Right,
            Axis::Right => Axis::Up,
            Axis::Up => Axis::Left,
        }
    }

    pub fn flip(self) -> Axis {
        match self {
            Axis::Left => Axis::Right,
            Axis::Right => Axis::Left,
            Axis::Down => Axis::Up,
            Axis::Up => Axis::Down,
        }
    }

    pub fn perpendicular(self, rhs: Axis) -> bool {
        self.is_horizontal() ^ rhs.is_horizontal()
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Axis::Left | Axis::Right)
    }

    pub fn is_vertical(self) -> bool {
        matches!(self, Axis::Up | Axis::Down)
    }

    pub fn is_positive(self) -> bool {
        matches!(self, Axis::Up | Axis::Right)
    }

    pub fn is_negative(self) -> bool {
        matches!(self, Axis::Left | Axis::Down)
    }

    pub fn sign(self) -> i32 {
        match self.is_positive() {
            true => 1,
            false => -1,
        }
    }

    pub fn positive(self) -> Axis {
        match self {
            Axis::Left | Axis::Right => Axis::Right,
            Axis::Down | Axis::Up => Axis::Up,
        }
    }

    pub fn negative(self) -> Axis {
        self.positive().flip()
    }

    pub fn horizontal_extend(self, extend: UVec2) -> UVec2 {
        match self.is_horizontal() {
            true => extend,
            false => UVec2::new(extend.y, extend.x),
        }
    }

    pub fn vertical_extend(self, extend: UVec2) -> UVec2 {
        match self.is_vertical() {
            true => extend,
            false => UVec2::new(extend.y, extend.x),
        }
    }

    pub fn horizontal_position(self, position: IVec2) -> IVec2 {
        match self.is_horizontal() {
            true => position,
            false => IVec2::new(position.y, position.x),
        }
    }

    pub fn vertical_position(self, position: IVec2) -> IVec2 {
        match self.is_vertical() {
            true => position,
            false => IVec2::new(position.y, position.x),
        }
    }
}
