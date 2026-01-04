use crate::{
    layout::Layout,
    measures::Rectangle,
    theme::{Attach, Theme},
    tools::pointer::{PointerCollider, PointerHover},
    widgets::events::Interact,
    world::{Descriptor, Element, Handle, World},
};

pub struct Panel {
    pub rect: Rectangle,
    pub order: isize,
    collider: Handle<PointerCollider>,
}

pub struct PanelDescriptor {
    pub rect: Rectangle,
    pub order: isize,
    pub theme: Option<Handle>,
}

impl Default for PanelDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 200, 200),
            order: -10,
            theme: None,
        }
    }
}

impl Descriptor for PanelDescriptor {
    type Target = Handle<Panel>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
        });

        let panel = world.insert(Panel {
            rect: self.rect,
            order: self.order,
            collider,
        });

        match self.theme {
            Some(theme) => world.queue(move |world| {
                world.trigger(theme, &Attach::<Panel>(panel));
            }),
            None => world.queue(move |world| {
                let theme = world.single::<Theme>().unwrap();
                world.trigger(theme, &Attach::<Panel>(panel));
            }),
        }

        panel
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
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
            Layout::Alpha(alpha) => unimplemented!(),
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
