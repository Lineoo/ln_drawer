use crate::{
    measures::Rectangle,
    tools::pointer::{PointerCollider, PointerHit, PointerHover, PointerStatus},
    widgets::events::{Click, Interact},
    world::{Descriptor, Element, Handle, World},
};

pub struct Button {
    pub rect: Rectangle,
    pub order: isize,
    collider: Handle<PointerCollider>,
}

pub struct ButtonDescriptor {
    pub rect: Rectangle,
    pub order: isize,
}

impl Default for ButtonDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            order: 10,
        }
    }
}

impl Descriptor for ButtonDescriptor {
    type Target = Handle<Button>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
        });

        world.insert(Button {
            rect: self.rect,
            order: self.order,
            collider,
        })
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.collider, move |event: &PointerHit, world, _| {
            if let PointerStatus::Release = event.status {
                world.trigger(this, &Click);
            }

            match event.status {
                PointerStatus::Press => {
                    world.trigger(this, &Interact::ButtonPress);
                }
                PointerStatus::Release => {
                    world.trigger(this, &Interact::ButtonRelease);
                }
                _ => {}
            }
        });

        world.observer(
            self.collider,
            move |event: &PointerHover, world, _| match event {
                PointerHover::Enter => {
                    world.trigger(this, &Interact::HoverEnter);
                }
                PointerHover::Leave => {
                    world.trigger(this, &Interact::HoverLeave);
                }
            },
        );

        world.dependency(self.collider, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let mut collider = world.fetch_mut(self.collider).unwrap();
        collider.order = self.order;
        collider.rect = self.rect;

        world.queue(move |world| {
            world.trigger(this, &Interact::PropertyChange);
        });
    }
}
