use crate::{
    measures::{Position, Rectangle},
    render::rounded::{RoundedRect, RoundedRectDescriptor},
    tools::pointer::{PointerEdge, PointerEdgeCollider, PointerHitEdge, PointerStatus},
    world::{Descriptor, Element, Handle, World},
};

/// !! EXPERIMENT ONLY !!
pub struct Panel {
    rounded: Handle<RoundedRect>,
    collider: Handle<PointerEdgeCollider>,
    start: Option<Start>,
}

#[derive(Debug, Clone, Copy)]
struct Start {
    cursor: Position,
    rect: Rectangle,
}

pub struct PanelDescriptor {
    pub rounded: RoundedRectDescriptor,
}

impl Descriptor for PanelDescriptor {
    type Target = Handle<Panel>;

    fn when_build(self, world: &World) -> Self::Target {
        let rounded = world.build(self.rounded);
        let collider = world.insert(PointerEdgeCollider {
            rect: self.rounded.rect,
            order: self.rounded.order,
        });

        world.insert(Panel {
            rounded,
            collider,
            start: None,
        })
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            self.collider,
            move |hit: &PointerHitEdge, world, collider| {
                let mut this = world.fetch_mut(this).unwrap();
                let mut rounded = world.fetch_mut(this.rounded).unwrap();

                match (hit.status, this.start) {
                    (PointerStatus::Press, None) => {
                        this.start = Some(Start {
                            cursor: hit.position,
                            rect: rounded.rect,
                        })
                    }

                    (PointerStatus::Moving, Some(start)) => {
                        let delta = hit.position - start.cursor;
                        match hit.edge {
                            PointerEdge::Leftdown => {
                                rounded.rect =
                                    start.rect.with_left_down(start.rect.left_down() + delta);
                            }
                            PointerEdge::Leftup => {
                                rounded.rect =
                                    start.rect.with_left_up(start.rect.left_up() + delta);
                            }
                            PointerEdge::Rightdown => {
                                rounded.rect =
                                    start.rect.with_right_down(start.rect.right_down() + delta);
                            }
                            PointerEdge::Rightup => {
                                rounded.rect =
                                    start.rect.with_right_up(start.rect.right_up() + delta);
                            }
                            PointerEdge::Left => {
                                rounded.rect = start.rect.with_left(start.rect.left() + delta.x);
                            }
                            PointerEdge::Down => {
                                rounded.rect = start.rect.with_down(start.rect.down() + delta.y);
                            }
                            PointerEdge::Right => {
                                rounded.rect = start.rect.with_right(start.rect.right() + delta.x);
                            }
                            PointerEdge::Up => {
                                rounded.rect = start.rect.with_up(start.rect.up() + delta.y);
                            }
                            PointerEdge::Body => {
                                rounded.rect = start.rect + delta;
                            }
                        }
                    }

                    (PointerStatus::Release, Some(_)) => {
                        this.start = None;
                    }

                    _ => unreachable!(),
                }

                let rect = rounded.rect;
                world.queue(move |world| {
                    let mut collider = world.fetch_mut(collider).unwrap();
                    collider.rect = rect;
                });
            },
        );
    }
}
