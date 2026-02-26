use crate::{
    layout::Layout,
    measures::Rectangle,
    theme::{Attach, Luni},
    tools::pointer::{PointerCollider, PointerHover, PointerMotion},
    widgets::events::{WidgetHover, WidgetModified},
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
}

impl Default for PanelDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 200, 200),
            order: -10,
        }
    }
}

impl Descriptor for PanelDescriptor {
    type Target = Handle<Panel>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
            enabled: false,
        });

        let panel = world.insert(Panel {
            rect: self.rect,
            order: self.order,
            collider,
        });

        world.insert(Attach {
            widget: panel,
            theme: world.single::<Luni>().unwrap(),
        });

        world.queue(move |world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = true;
        });

        panel
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
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
