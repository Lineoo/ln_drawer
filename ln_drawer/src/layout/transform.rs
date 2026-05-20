use ln_world::{Element, Handle, World};

use crate::{
    measures::Rectangle,
    widgets::{WidgetAnimatedRectangle, WidgetRectangle},
};

pub struct Transform {
    pub value: TransformValue,
    pub source: Handle,
    pub target: Handle,
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
    pub const fn copy() -> TransformValue {
        TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 0,
            },
            down: TransformEdge {
                anchor: 0.0,
                offset: 0,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: 0,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: 0,
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
        let left = source.extend.w as f32 * self.left.anchor;
        let left = source.origin.x + left.round() as i32 + self.left.offset;

        let down = source.extend.h as f32 * self.down.anchor;
        let down = source.origin.y + down.round() as i32 + self.down.offset;

        let right = source.extend.w as f32 * self.right.anchor;
        let right = source.origin.x + right.round() as i32 + self.right.offset;

        let up = source.extend.h as f32 * self.up.anchor;
        let up = source.origin.y + up.round() as i32 + self.up.offset;

        Rectangle::new(left, down, right, up)
    }
}

impl Element for Transform {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let ob = world.observer(self.source, move |&WidgetRectangle(rect), world| {
            let this = world.fetch(this).unwrap();
            let target = this.value.compute(rect);

            world.queue_trigger(this.target, WidgetRectangle(target));
        });

        let oba = world.observer(self.source, move |&WidgetAnimatedRectangle(rect), world| {
            let this = world.fetch(this).unwrap();
            let target = this.value.compute(rect);

            world.queue_trigger(this.target, WidgetRectangle(target));
        });

        world.dependency(ob, this);
        world.dependency(oba, this);
        world.dependency(this, self.source);
        world.dependency(this, self.target);
    }
}
