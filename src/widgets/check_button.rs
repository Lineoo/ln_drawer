use crate::{
    layout::LayoutRectangle,
    measures::Rectangle,
    theme::Luni,
    tools::pointer::{
        PointerCollider, PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus,
    },
    widgets::{Attach, WidgetButton, WidgetChecked, WidgetClick, WidgetHover, WidgetRectangle},
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
            enabled: false,
        });

        let button = world.insert(CheckButton {
            rect: self.rect,
            order: self.order,
            checked: self.checked,
            collider,
        });

        world.insert(Attach {
            widget: button,
            target: world.single::<Luni>().unwrap(),
        });

        world.queue(move |world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = true;
        });

        button
    }
}

impl Element for CheckButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.collider, move |event: &PointerHit, world, _| {
            if let PointerHitStatus::Release = event.status {
                world.trigger(this, &WidgetClick);
            }

            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerHitStatus::Release => {
                    world.trigger(this, &WidgetButton::ButtonRelease);
                }
                _ => {}
            }
        });

        world.observer(
            self.collider,
            move |event: &PointerHover, world, _| match event.motion {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Moving => {}
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
            },
        );

        world.observer(this, |&LayoutRectangle(rect), world, this| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.dependency(self.collider, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let mut collider = world.fetch_mut(self.collider).unwrap();
        collider.order = self.order;
        collider.rect = self.rect;

        world.trigger(this, &WidgetRectangle(self.rect));
        world.trigger(this, &WidgetChecked(self.checked));
    }
}
