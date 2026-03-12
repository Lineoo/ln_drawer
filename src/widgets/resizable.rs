use crate::{
    layout::LayoutRectangle,
    measures::{Position, Rectangle},
    theme::Luni,
    tools::pointer::{PointerEdge, PointerEdgeCollider, PointerHitEdge, PointerHitStatus},
    widgets::{Attach, WidgetDestroyed, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct Resizable {
    pub rect: Rectangle,
}

#[derive(Clone, Copy)]
struct Start {
    cursor: Position,
    rect: Rectangle,
}

impl Element for Resizable {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&LayoutRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.insert(Attach {
            widget: this,
            target: world.single::<Luni>().unwrap(),
        });

        self.attach_pointer(world, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetRectangle(self.rect));
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetDestroyed);
    }
}

impl Resizable {
    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerEdgeCollider {
            rect: self.rect,
            order: 10,
            enabled: true,
        });

        world.dependency(collider, this);

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = rect;
        });

        let mut start = None::<Start>;
        world.observer(collider, move |hit: &PointerHitEdge, world| {
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
                    match hit.edge {
                        PointerEdge::Leftdown => {
                            this.rect = this.rect.with_left_down(start.rect.left_down() + delta);
                        }
                        PointerEdge::Leftup => {
                            this.rect = this.rect.with_left_up(start.rect.left_up() + delta);
                        }
                        PointerEdge::Rightdown => {
                            this.rect = this.rect.with_right_down(start.rect.right_down() + delta);
                        }
                        PointerEdge::Rightup => {
                            this.rect = this.rect.with_right_up(start.rect.right_up() + delta);
                        }
                        PointerEdge::Left => {
                            this.rect = this.rect.with_left(start.rect.left() + delta.x);
                        }
                        PointerEdge::Down => {
                            this.rect = this.rect.with_down(start.rect.down() + delta.y);
                        }
                        PointerEdge::Right => {
                            this.rect = this.rect.with_right(start.rect.right() + delta.x);
                        }
                        PointerEdge::Up => {
                            this.rect = this.rect.with_up(start.rect.up() + delta.y);
                        }
                        PointerEdge::Body => {
                            this.rect = start.rect + delta;
                        }
                    }
                }

                (PointerHitStatus::Release, Some(_)) => {
                    start = None;
                }

                _ => unreachable!(),
            }
        });
    }
}
