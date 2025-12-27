use crate::{
    layout::events::LayoutRect,
    measures::{Position, Rectangle},
    tools::pointer::{
        PointerEdge, PointerEdgeCollider, PointerHitEdge, PointerHitEdgeCheck, PointerStatus,
    },
    world::{Descriptor, Element, Handle, World},
};

pub struct Translatable {
    hollow: bool,
    target: Handle,
    start: Option<Start>,
    collider: Handle<PointerEdgeCollider>,
}

pub struct TranslatableDescriptor {
    pub rect: Rectangle,
    pub order: isize,
    pub hollow: bool,
    pub target: Handle,
}

#[derive(Debug, Clone, Copy)]
struct Start {
    cursor: Position,
    rect: Rectangle,
}

impl Descriptor for TranslatableDescriptor {
    type Target = Handle<Translatable>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerEdgeCollider {
            rect: self.rect,
            order: self.order,
        });

        world.insert(Translatable {
            hollow: self.hollow,
            target: self.target,
            start: None,
            collider,
        })
    }
}

impl Element for Translatable {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            self.collider,
            move |check: &PointerHitEdgeCheck, world, _| {
                let this = world.fetch(this).unwrap();
                if this.hollow && check.edge == PointerEdge::Body {
                    check.occlude.set(false);
                }
            },
        );

        world.observer(
            self.collider,
            move |hit: &PointerHitEdge, world, collider| {
                let mut this = world.fetch_mut(this).unwrap();
                let mut collider = world.fetch_mut(collider).unwrap();

                match (hit.status, this.start) {
                    (PointerStatus::Press, None) => {
                        this.start = Some(Start {
                            cursor: hit.position,
                            rect: collider.rect,
                        })
                    }

                    (PointerStatus::Moving, Some(start)) => {
                        let delta = hit.position - start.cursor;
                        collider.rect = start.rect + delta;
                    }

                    (PointerStatus::Release, Some(_)) => {
                        this.start = None;
                    }

                    _ => unreachable!(),
                }

                world.trigger(this.target, &LayoutRect(collider.rect));
            },
        );
    }
}
