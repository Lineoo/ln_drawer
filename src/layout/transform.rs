use crate::{
    layout::Layout,
    measures::{Position, Rectangle},
    world::{Element, Handle, World},
};

pub struct Transform {
    pub left: TransformEdge,
    pub down: TransformEdge,
    pub right: TransformEdge,
    pub up: TransformEdge,

    pub source: Handle,
    pub target: Handle,
}

pub struct TransformEdge {
    pub anchor: f32,
    pub offset: i32,
}

impl Transform {
    pub fn anchor(
        anchor: (f32, f32),
        rect: Rectangle,
        offset: Position,
        source: Handle,
        target: Handle,
    ) -> Transform {
        Transform {
            left: TransformEdge {
                anchor: anchor.0,
                offset: offset.x,
            },
            down: TransformEdge {
                anchor: anchor.1,
                offset: offset.y,
            },
            right: TransformEdge {
                anchor: anchor.0,
                offset: offset.x + rect.width() as i32,
            },
            up: TransformEdge {
                anchor: anchor.1,
                offset: offset.y + rect.height() as i32,
            },
            source,
            target,
        }
    }
}

impl Element for Transform {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let ob = world.observer(self.source, move |layout: &Layout, world, _| match layout {
            Layout::Rectangle(rect) => {
                let this = world.fetch(this).unwrap();

                let left = rect.extend.w as f32 * this.left.anchor;
                let left = rect.origin.x + left.round() as i32 + this.left.offset;

                let down = rect.extend.h as f32 * this.down.anchor;
                let down = rect.origin.y + down.round() as i32 + this.down.offset;

                let right = rect.extend.w as f32 * this.right.anchor;
                let right = rect.origin.x + right.round() as i32 + this.right.offset;

                let up = rect.extend.h as f32 * this.up.anchor;
                let up = rect.origin.y + up.round() as i32 + this.up.offset;

                let layout = Layout::Rectangle(Rectangle::new(left, down, right, up));
                world.trigger(this.target, &layout);
            }
            Layout::Alpha(alpha) => {
                let this = world.fetch(this).unwrap();
                let layout = Layout::Alpha(*alpha);
                world.trigger(this.target, &layout);
            }
        });

        world.dependency(ob, this);
    }
}
