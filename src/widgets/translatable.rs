use crate::{
    layout::{LayoutControl, LayoutControls},
    measures::{Position, Rectangle},
    render::rounded::RoundedRectDescriptor,
    theme::Luni,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::{WidgetDestroyed, WidgetEnabled, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct Translatable {
    pub rect: Rectangle,
}

impl Translatable {
    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(LayoutControl {
            rectangle: Some(Box::new(move |world, rect| {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = rect;
                world.queue_trigger(this.handle(), WidgetRectangle(rect));
                rect
            })),
            enabled: Some(Box::new(move |world, enabled| {
                world.queue_trigger(this, WidgetEnabled(enabled));
            })),
        });

        let mut layouts = world.single_fetch_mut::<LayoutControls>().unwrap();
        layouts.0.insert(this.untyped(), control);

        world.dependency(control, this);
    }

    fn attach_luni(&mut self, world: &World, this: Handle<Self>) {
        let luni = world.single_fetch::<Luni>().unwrap();
        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            visible: true,
            order: 110,
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });

        world.observer(this, move |&WidgetEnabled(enabled), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.visible = enabled;
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 110,
            enabled: true,
        });

        world.dependency(collider, this);
        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = rect;
        });

        world.observer(this, move |&WidgetEnabled(enabled), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = enabled;
        });

        #[derive(Clone, Copy)]
        struct Start {
            cursor: Position,
            rect: Rectangle,
        }

        let mut start = None;
        world.observer(collider, move |hit: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();

            match (hit.status, start) {
                (PointerHitStatus::Press, None) => {
                    start = Some(Start {
                        cursor: hit.position,
                        rect: this.rect,
                    });
                }

                (PointerHitStatus::Moving, Some(start)) => {
                    let delta = hit.position - start.cursor;
                    this.rect = start.rect + delta;
                    world.queue_trigger(this.handle(), WidgetRectangle(this.rect));
                }

                (PointerHitStatus::Release, Some(_)) => {
                    start = None;
                }

                _ => unreachable!(),
            }
        });
    }
}

impl Element for Translatable {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_layout(world, this);
        self.attach_luni(world, this);
        self.attach_pointer(world, this);
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetDestroyed);
    }
}
