use crate::{
    layout::events::LayoutRect,
    measures::{Position, Rectangle},
    tools::pointer::{PointerEdge, PointerEdgeCollider, PointerHitEdge, PointerStatus},
    world::{Descriptor, Element, Handle, World},
};

pub struct Resizable {
    collider: Handle<PointerEdgeCollider>,
    target: Handle,
    start: Option<Start>,
}

pub struct ResizableDescriptor {
    pub rect: Rectangle,
    pub order: isize,
    pub target: Handle,
}

#[derive(Debug, Clone, Copy)]
struct Start {
    cursor: Position,
    rect: Rectangle,
}

impl Descriptor for ResizableDescriptor {
    type Target = Handle<Resizable>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerEdgeCollider {
            rect: self.rect,
            order: self.order,
        });

        world.insert(Resizable {
            collider,
            target: self.target,
            start: None,
        })
    }
}

impl Element for Resizable {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
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
                        match hit.edge {
                            PointerEdge::Leftdown => {
                                collider.rect =
                                    start.rect.with_left_down(start.rect.left_down() + delta);
                            }
                            PointerEdge::Leftup => {
                                collider.rect =
                                    start.rect.with_left_up(start.rect.left_up() + delta);
                            }
                            PointerEdge::Rightdown => {
                                collider.rect =
                                    start.rect.with_right_down(start.rect.right_down() + delta);
                            }
                            PointerEdge::Rightup => {
                                collider.rect =
                                    start.rect.with_right_up(start.rect.right_up() + delta);
                            }
                            PointerEdge::Left => {
                                collider.rect = start.rect.with_left(start.rect.left() + delta.x);
                            }
                            PointerEdge::Down => {
                                collider.rect = start.rect.with_down(start.rect.down() + delta.y);
                            }
                            PointerEdge::Right => {
                                collider.rect = start.rect.with_right(start.rect.right() + delta.x);
                            }
                            PointerEdge::Up => {
                                collider.rect = start.rect.with_up(start.rect.up() + delta.y);
                            }
                            PointerEdge::Body => {
                                collider.rect = start.rect + delta;
                            }
                        }
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
