use crate::{
    layout::Layout,
    measures::Rectangle,
    tools::pointer::{PointerCollider, PointerHit, PointerHover, PointerStatus},
    widgets::events::{Click, Interact, Switch},
    world::{Descriptor, Element, Handle, World},
};

pub struct CheckButton {
    pub rect: Rectangle,
    pub checked: bool,
    pub order: isize,
    collider: Handle<PointerCollider>,
}

pub struct CheckButtonDescriptor {
    pub rect: Rectangle,
    pub checked: bool,
    pub order: isize,
}

impl Default for CheckButtonDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            checked: false,
            order: 10,
        }
    }
}

impl Descriptor for CheckButtonDescriptor {
    type Target = Handle<CheckButton>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
        });

        world.insert(CheckButton {
            rect: self.rect,
            order: self.order,
            checked: self.checked,
            collider,
        })
    }
}

impl Element for CheckButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.collider, move |event: &PointerHit, world, _| {
            if let PointerStatus::Release = event.status {
                world.trigger(this, &Switch);
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

        world.observer(this, |layout: &Layout, world, this| match layout {
            Layout::Rectangle(rect) => {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = *rect;
            }
            Layout::Alpha(alpha) => unimplemented!()
        });

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
