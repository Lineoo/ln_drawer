use crate::{
    layout::LayoutRectangle,
    measures::Rectangle,
    theme::Luni,
    tools::pointer::{PointerCollider, PointerHover, PointerHoverStatus},
    widgets::{Attach, WidgetDestroyed, WidgetHover, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct Panel {
    pub rect: Rectangle,
    pub order: isize,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 200, 200),
            order: -10,
        }
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.insert(Attach {
            widget: this,
            target: world.single::<Luni>().unwrap(),
        });

        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: self.order,
            enabled: false,
        });

        world.observer(collider, move |event: &PointerHover, world| {
            match event.motion {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Moving => {}
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
            }
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = rect;
        });

        world.observer(this, move |&LayoutRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.dependency(collider, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetRectangle(self.rect));
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetDestroyed);
    }
}
