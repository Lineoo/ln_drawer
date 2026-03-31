use crate::{
    layout::{LayoutControls, LayoutRectangle},
    measures::{Position, Rectangle},
    widgets::WidgetRectangle,
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
    pub const fn copy(source: Handle, target: Handle) -> Transform {
        Transform {
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
            source,
            target,
        }
    }

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
        let ob = world.observer(self.source, move |&WidgetRectangle(rect), world| {
            let this = world.fetch(this).unwrap();

            let left = rect.extend.w as f32 * this.left.anchor;
            let left = rect.origin.x + left.round() as i32 + this.left.offset;

            let down = rect.extend.h as f32 * this.down.anchor;
            let down = rect.origin.y + down.round() as i32 + this.down.offset;

            let right = rect.extend.w as f32 * this.right.anchor;
            let right = rect.origin.x + right.round() as i32 + this.right.offset;

            let up = rect.extend.h as f32 * this.up.anchor;
            let up = rect.origin.y + up.round() as i32 + this.up.offset;

            let layout = LayoutRectangle(Rectangle::new(left, down, right, up));
            world.trigger(this.target, &layout);

            let controls = world.single_fetch::<LayoutControls>().unwrap();
            if let Some(&control) = controls.0.get(&this.target)
                && let Some(rect) = &mut world.fetch_mut(control).unwrap().rectangle
            {
                (rect)(world, Rectangle::new(left, down, right, up));
            }
        });

        world.dependency(ob, this);
        world.dependency(this, self.source);
        world.dependency(this, self.target);
    }
}
