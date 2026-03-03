use crate::{
    layout::{LayoutRectangle, transform::Transform},
    measures::Rectangle,
    theme::Luni,
    tools::pointer::{
        PointerCollider, PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus,
    },
    widgets::{Attach, WidgetButton, WidgetClick, WidgetHover, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct Button {
    pub rect: Rectangle,
    pub order: isize,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            order: 10,
        }
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |&LayoutRectangle(rect), world, this| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.insert(Attach {
            widget: this,
            target: world.single::<Luni>().unwrap(),
        });

        self.attach_pointer(world, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetRectangle(self.rect));
    }
}

impl Button {
    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
            enabled: true,
        });

        world.insert(Transform::copy(this.untyped(), collider.untyped()));

        world.observer(collider, move |event: &PointerHit, world, _| {
            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerHitStatus::Release => {
                    world.trigger(this, &WidgetClick);
                    world.trigger(this, &WidgetButton::ButtonRelease);
                }
                _ => {}
            }
        });

        world.observer(
            collider,
            move |event: &PointerHover, world, _| match event.motion {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
                _ => {}
            },
        );

        world.dependency(collider, this);
    }
}
