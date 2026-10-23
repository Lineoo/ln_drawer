use ln_world::{Descriptor, Handle, World};

use crate::{
    layout::{transfer::Transfer, val::Val},
    measures::Rectangle,
};

pub struct Transform {
    pub value: TransformValue,
    pub source: Handle<Val<Rectangle>>,
}

impl Transform {
    pub fn new(value: TransformValue, source: Handle<Val<Rectangle>>) -> Self {
        Transform { value, source }
    }

    pub fn anchor(anchor: (f32, f32), rect: Rectangle, source: Handle<Val<Rectangle>>) -> Self {
        Transform::new(TransformValue::anchor(anchor, rect), source)
    }
}

#[derive(Clone, Copy)]
pub struct TransformValue {
    pub left: TransformEdge,
    pub down: TransformEdge,
    pub right: TransformEdge,
    pub up: TransformEdge,
}

#[derive(Clone, Copy)]
pub struct TransformEdge {
    pub anchor: f32,
    pub offset: i32,
}

impl TransformValue {
    pub const fn shift(x: i32, y: i32) -> TransformValue {
        TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: x,
            },
            down: TransformEdge {
                anchor: 0.0,
                offset: y,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: x,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: y,
            },
        }
    }

    pub const fn anchor(anchor: (f32, f32), rect: Rectangle) -> TransformValue {
        TransformValue {
            left: TransformEdge {
                anchor: anchor.0,
                offset: rect.left(),
            },
            down: TransformEdge {
                anchor: anchor.1,
                offset: rect.down(),
            },
            right: TransformEdge {
                anchor: anchor.0,
                offset: rect.right(),
            },
            up: TransformEdge {
                anchor: anchor.1,
                offset: rect.up(),
            },
        }
    }

    pub const fn shrink(width: i32, height: i32) -> TransformValue {
        TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: width,
            },
            down: TransformEdge {
                anchor: 0.0,
                offset: height,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: -width,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: -height,
            },
        }
    }

    #[expect(unused)]
    pub const fn scale(width: f32, height: f32) -> TransformValue {
        TransformValue {
            left: TransformEdge {
                anchor: 0.5 - width * 0.5,
                offset: 0,
            },
            down: TransformEdge {
                anchor: 0.5 - height * 0.5,
                offset: 0,
            },
            right: TransformEdge {
                anchor: 0.5 + width * 0.5,
                offset: 0,
            },
            up: TransformEdge {
                anchor: 0.5 + height * 0.5,
                offset: 0,
            },
        }
    }

    pub fn compute(&self, source: Rectangle) -> Rectangle {
        let left = source.extend.x as f32 * self.left.anchor;
        let left = source.origin.x + left.round() as i32 + self.left.offset;

        let down = source.extend.y as f32 * self.down.anchor;
        let down = source.origin.y + down.round() as i32 + self.down.offset;

        let right = source.extend.x as f32 * self.right.anchor;
        let right = source.origin.x + right.round() as i32 + self.right.offset;

        let up = source.extend.y as f32 * self.up.anchor;
        let up = source.origin.y + up.round() as i32 + self.up.offset;

        Rectangle::new(left, down, right, up)
    }
}

impl Descriptor for Transform {
    type Target = Handle<Val<Rectangle>>;

    fn when_build(self, world: &World) -> Self::Target {
        Transfer::derive(world, self.source).build(move |&source| self.value.compute(source))
    }
}
