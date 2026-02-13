use crate::{
    layout::Layout,
    measures::Rectangle,
    theme::{Attach, Theme},
    tools::pointer::{PointerCollider, PointerHit, PointerHover, PointerMotion, PointerStatus},
    widgets::events::{WidgetButton, WidgetClick, WidgetHover, WidgetModified},
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
    pub theme: Option<Handle>,
}

impl Default for ButtonDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            order: 10,
            theme: None,
        }
    }
}

impl Descriptor for ButtonDescriptor {
    type Target = Handle<Button>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
            enabled: false,
        });

        let button = world.insert(Button {
            rect: self.rect,
            order: self.order,
            collider,
        });

        match self.theme {
            Some(theme) => world.queue(move |world| {
                world.trigger(theme, &Attach::<Button>(button));
            }),
            None => world.queue(move |world| {
                let theme = world.single::<Theme>().unwrap();
                world.trigger(theme, &Attach::<Button>(button));
            }),
        }

        world.queue(move |world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = true;
        });

        button
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.collider, move |event: &PointerHit, world, _| {
            if let PointerStatus::Release = event.status {
                world.trigger(this, &WidgetClick);
            }

            match event.status {
                PointerStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerStatus::Release => {
                    world.trigger(this, &WidgetButton::ButtonRelease);
                }
                _ => {}
            }
        });

        world.observer(
            self.collider,
            move |event: &PointerHover, world, _| match event.motion {
                PointerMotion::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerMotion::Moving => {}
                PointerMotion::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
            },
        );

        world.observer(this, |layout: &Layout, world, this| match layout {
            Layout::Rectangle(rect) => {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = *rect;
            }
            Layout::Alpha(alpha) => unimplemented!(),
        });

        world.dependency(self.collider, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let mut collider = world.fetch_mut(self.collider).unwrap();
        collider.order = self.order;
        collider.rect = self.rect;

        world.queue(move |world| {
            world.trigger(this, &WidgetModified);
        });
    }
}
