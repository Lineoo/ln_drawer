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
}
