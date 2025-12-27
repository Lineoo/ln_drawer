use crate::measures::Position;

pub struct Transform {
    pub leftdown: TransformCorner,
    pub leftup: TransformCorner,
    pub rightdown: TransformCorner,
    pub rightup: TransformCorner,
}

pub struct TransformCorner {
    pub anchor: (f32, f32),
    pub offset: Position,
}
