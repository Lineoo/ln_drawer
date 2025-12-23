use crate::{
    measures::{Position, Rectangle},
    render::rounded::{RoundedRect, RoundedRectDescriptor},
    tools::pointer::{PointerEdge, PointerEdgeCollider, PointerHit, PointerHitEdge},
    world::{Descriptor, Element, Handle, World},
};

/// !! EXPERIMENT ONLY !!
pub struct Panel {
    rounded: RoundedRect,
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

    fn build(self, world: &World) -> Self::Target {
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

                match (hit.hit, this.start) {
                    (PointerHit::Pressed(position), None) => {
                        this.start = Some(Start {
                            cursor: position,
                            rect: this.rounded.rect,
                        })
                    }

                    (PointerHit::Moving(position), Some(start)) => {
                        let delta = position - start.cursor;
                        match hit.edge {
                            PointerEdge::Leftdown => {
                                this.rounded.rect =
                                    start.rect.with_left_down(start.rect.left_down() + delta);
                            }
                            PointerEdge::Leftup => {
                                this.rounded.rect =
                                    start.rect.with_left_up(start.rect.left_up() + delta);
                            }
                            PointerEdge::Rightdown => {
                                this.rounded.rect =
                                    start.rect.with_right_down(start.rect.right_down() + delta);
                            }
                            PointerEdge::Rightup => {
                                this.rounded.rect =
                                    start.rect.with_right_up(start.rect.right_up() + delta);
                            }
                            PointerEdge::Left => {
                                this.rounded.rect =
                                    start.rect.with_left(start.rect.left() + delta.x);
                            }
                            PointerEdge::Down => {
                                this.rounded.rect =
                                    start.rect.with_down(start.rect.down() + delta.y);
                            }
                            PointerEdge::Right => {
                                this.rounded.rect =
                                    start.rect.with_right(start.rect.right() + delta.x);
                            }
                            PointerEdge::Up => {
                                this.rounded.rect = start.rect.with_up(start.rect.up() + delta.y);
                            }
                            PointerEdge::Body => {
                                this.rounded.rect = start.rect + delta;
                            }
                        }
                    }

                    (PointerHit::Released(_), Some(_)) => {
                        this.start = None;
                    }

                    _ => unreachable!(),
                }

                this.rounded.upload();

                let rect = this.rounded.rect;
                world.queue(move |world| {
                    let mut collider = world.fetch_mut(collider).unwrap();
                    collider.rect = rect;
                });
            },
        );
    }
}
