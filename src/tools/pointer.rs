use std::cell::Cell;

use winit::event::{ButtonSource, ElementState, MouseButton, PointerKind, WindowEvent};

use crate::{
    layout::LayoutRectangle,
    lnwin::Lnwindow,
    measures::{Position, PositionFract, Rectangle, Size},
    render::viewport::Viewport,
    world::{Element, Handle, World},
};

#[derive(Debug, Default)]
pub struct PointerTool {
    position: PositionFract,
    hovering: Option<Handle<PointerCollider>>,
    pressed: bool,
}

/// See [`PointerColliderEdge`] for a more specific version for frame ops.
///
/// **Event associated**: [`PointerHit`], [`PointerMenu`], [`PointerEnter`], [`PointerLeave`]
#[derive(Debug, Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

/// Similar to [`PointerCollider`], but will react when mouse hover on
/// its edge and provide detailed information on which edge it hit.
///
/// **Event associated**: [`PointerHitEdge`]
#[derive(Debug, Clone, Copy)]
pub struct PointerEdgeCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

// Events //

#[derive(Debug, Clone, Copy)]
pub struct PointerHit {
    pub position: Position,
    pub status: PointerHitStatus,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHitEdge {
    pub position: Position,
    pub status: PointerHitStatus,
    pub edge: PointerEdge,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHover {
    pub position: Position,
    pub motion: PointerHoverStatus,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHoverEdge {
    pub position: Position,
    pub motion: PointerHoverStatus,
    pub edge: PointerEdge,
}

#[derive(Debug)]
pub struct PointerCheck {
    pub position: Position,
    pub occlude: Cell<bool>,
}

#[derive(Debug)]
pub struct PointerEdgeCheck {
    pub position: Position,
    pub edge: PointerEdge,
    pub occlude: Cell<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHitStatus {
    Press,
    Moving,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHoverStatus {
    Enter,
    Moving,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEdge {
    Leftdown,
    Leftup,
    Rightdown,
    Rightup,

    Left,
    Down,
    Right,
    Up,

    Body,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerMenu(pub Position);

// Inner Implements //

struct ColliderUpdate;
struct ColliderEdgeLock {
    edge: Option<PointerEdge>,
}

impl Element for ColliderEdgeLock {}

// Functions //

impl PointerCollider {
    pub const fn fullscreen(order: isize) -> PointerCollider {
        PointerCollider {
            rect: Rectangle {
                origin: Position::MIN,
                extend: Size::MAX,
            },
            order,
            enabled: true,
        }
    }
}

// Behaviors //

impl Element for PointerCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |&LayoutRectangle(rect), world, this| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.queue(|world| {
            if let Ok(mut pointer) = world.single_fetch_mut::<PointerTool>() {
                pointer.update_hovering(world, PointerKind::Unknown);
            }
        });
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        world.queue(|world| {
            if let Ok(mut pointer) = world.single_fetch_mut::<PointerTool>() {
                pointer.update_hovering(world, PointerKind::Unknown);
            }
        });
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        world.queue(|world| {
            if let Ok(mut pointer) = world.single_fetch_mut::<PointerTool>() {
                pointer.update_hovering(world, PointerKind::Unknown);
            }
        });
    }
}

impl Element for PointerEdgeCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        const EXPAND: i32 = 5;

        let collider = world.insert(PointerCollider {
            rect: self.rect.expand(EXPAND),
            order: self.order,
            enabled: true,
        });

        let lock = world.insert(ColliderEdgeLock { edge: None });

        world.observer(collider, move |event: &PointerCheck, world, _| {
            let mut idx = 0;

            let fetched = world.fetch(this).unwrap();
            let shrink = fetched.rect.expand(-EXPAND);
            drop(fetched);

            if event.position.x < shrink.left() {
                idx += 0;
            } else if event.position.x < shrink.right() {
                idx += 1;
            } else {
                idx += 2;
            }

            if event.position.y < shrink.down() {
                idx += 0;
            } else if event.position.y < shrink.up() {
                idx += 3;
            } else {
                idx += 6;
            }

            let edge = match idx {
                0 => PointerEdge::Leftdown,
                1 => PointerEdge::Down,
                2 => PointerEdge::Rightdown,
                3 => PointerEdge::Left,
                4 => PointerEdge::Body,
                5 => PointerEdge::Right,
                6 => PointerEdge::Leftup,
                7 => PointerEdge::Up,
                8 => PointerEdge::Rightup,
                _ => unreachable!(),
            };

            let check = PointerEdgeCheck {
                position: event.position,
                edge,
                occlude: event.occlude.clone(),
            };

            world.trigger(this, &check);

            event.occlude.set(check.occlude.get());
        });

        world.observer(collider, move |event: &PointerHit, world, _| {
            let mut lock = world.fetch_mut(lock).unwrap();

            match (event.status, lock.edge) {
                (PointerHitStatus::Press, None) => {
                    let mut idx = 0;

                    let fetched = world.fetch(this).unwrap();
                    let shrink = fetched.rect.expand(-EXPAND);
                    drop(fetched);

                    if event.position.x < shrink.left() {
                        idx += 0;
                    } else if event.position.x < shrink.right() {
                        idx += 1;
                    } else {
                        idx += 2;
                    }

                    if event.position.y < shrink.down() {
                        idx += 0;
                    } else if event.position.y < shrink.up() {
                        idx += 3;
                    } else {
                        idx += 6;
                    }

                    let edge = match idx {
                        0 => PointerEdge::Leftdown,
                        1 => PointerEdge::Down,
                        2 => PointerEdge::Rightdown,
                        3 => PointerEdge::Left,
                        4 => PointerEdge::Body,
                        5 => PointerEdge::Right,
                        6 => PointerEdge::Leftup,
                        7 => PointerEdge::Up,
                        8 => PointerEdge::Rightup,
                        _ => unreachable!(),
                    };

                    lock.edge = Some(edge);
                    world.trigger(
                        this,
                        &PointerHitEdge {
                            position: event.position,
                            status: event.status,
                            edge,
                        },
                    );
                }

                (PointerHitStatus::Moving, Some(edge)) => {
                    world.trigger(
                        this,
                        &PointerHitEdge {
                            position: event.position,
                            status: event.status,
                            edge,
                        },
                    );
                }

                (PointerHitStatus::Release, Some(edge)) => {
                    lock.edge = None;
                    world.trigger(
                        this,
                        &PointerHitEdge {
                            position: event.position,
                            status: event.status,
                            edge,
                        },
                    );
                }

                _ => unreachable!(),
            }
        });

        world.observer(this, move |ColliderUpdate, world, this| {
            let this = world.fetch(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = this.rect.expand(EXPAND);
        });

        world.dependency(collider, this);
        world.dependency(lock, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        // will flow to the raw PointerCollider
        world.queue(move |world| {
            world.trigger(this, &ColliderUpdate);
        });
    }
}

impl Element for PointerTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            world.single::<Lnwindow>().unwrap(),
            move |event: &WindowEvent, world, lnwindow| {
                let mut pointer = world.fetch_mut(this).unwrap();
                let lnwindow = world.fetch(lnwindow).unwrap();
                match event {
                    WindowEvent::PointerMoved {
                        position,
                        source,
                        primary: true,
                        ..
                    } => {
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        let position = lnwindow.cursor_to_screen(*position);
                        let position = viewport.screen_to_world_absolute(position);

                        let kind = PointerKind::from(source.clone());

                        pointer.position = position;
                        pointer.update_hovering(world, kind);

                        if let Some(hovering) = pointer.hovering {
                            if pointer.pressed {
                                world.trigger(
                                    hovering,
                                    &PointerHit {
                                        position: position.floor(),
                                        status: PointerHitStatus::Moving,
                                        pointer: kind,
                                    },
                                );
                            } else {
                                world.trigger(
                                    hovering,
                                    &PointerHover {
                                        position: position.floor(),
                                        motion: PointerHoverStatus::Moving,
                                        pointer: kind,
                                    },
                                );
                            }
                        }
                    }

                    WindowEvent::PointerButton {
                        position,
                        button,
                        state,
                        primary: true,
                        ..
                    } if matches!(
                        button,
                        ButtonSource::Mouse(MouseButton::Left)
                            | ButtonSource::Touch { .. }
                            | ButtonSource::TabletTool { .. }
                            | ButtonSource::Unknown(_)
                    ) =>
                    {
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        let position = lnwindow.cursor_to_screen(*position);
                        let position = viewport.screen_to_world_absolute(position);

                        let kind = match button {
                            ButtonSource::Mouse(_) => PointerKind::Mouse,
                            ButtonSource::Touch { finger_id, .. } => PointerKind::Touch(*finger_id),
                            ButtonSource::TabletTool { kind, .. } => PointerKind::TabletTool(*kind),
                            ButtonSource::Unknown(_) => PointerKind::Unknown,
                        };

                        pointer.position = position;
                        pointer.update_hovering(world, kind);

                        match state {
                            ElementState::Pressed => {
                                if let Some(hovering) = pointer.hovering {
                                    world.trigger(
                                        hovering,
                                        &PointerHit {
                                            position: pointer.position.floor(),
                                            status: PointerHitStatus::Press,
                                            pointer: kind,
                                        },
                                    );
                                }

                                pointer.pressed = true;
                            }
                            ElementState::Released => {
                                if let Some(hovering) = pointer.hovering {
                                    world.trigger(
                                        hovering,
                                        &PointerHit {
                                            position: pointer.position.floor(),
                                            status: PointerHitStatus::Release,
                                            pointer: kind,
                                        },
                                    );
                                }

                                pointer.pressed = false;
                            }
                        }
                    }

                    WindowEvent::PointerButton {
                        button: ButtonSource::Mouse(MouseButton::Right),
                        state: ElementState::Pressed,
                        primary: true,
                        ..
                    } => {
                        let target = intersect(world, pointer.position.floor()).first().copied();
                        if let Some(target) = target {
                            world.trigger(target, &PointerMenu(pointer.position.floor()));
                        }
                    }

                    WindowEvent::PointerLeft { primary: true, .. } => {
                        if let Some(hovering) = pointer.hovering {
                            world.trigger(
                                hovering,
                                &PointerHover {
                                    position: pointer.position.floor(),
                                    motion: PointerHoverStatus::Leave,
                                    pointer: PointerKind::Mouse,
                                },
                            );
                        }

                        pointer.hovering = None;
                    }

                    _ => {}
                }
            },
        );
    }
}

impl PointerTool {
    fn update_hovering(&mut self, world: &World, pointer: PointerKind) {
        let position = self.position;

        if self.pressed {
            return;
        }

        let mut landing = None;
        for each in intersect(world, position.floor()) {
            let check = PointerCheck {
                position: position.floor(),
                occlude: Cell::new(true),
            };
            world.trigger(each, &check);
            if check.occlude.get() {
                landing = Some(each);
                break;
            }
        }

        if self.hovering != landing {
            if let Some(hovering) = self.hovering {
                world.trigger(
                    hovering,
                    &PointerHover {
                        position: position.floor(),
                        motion: PointerHoverStatus::Leave,
                        pointer,
                    },
                );
            }

            if let Some(landing) = landing {
                world.trigger(
                    landing,
                    &PointerHover {
                        position: position.floor(),
                        motion: PointerHoverStatus::Enter,
                        pointer,
                    },
                );
            }

            self.hovering = landing;
        }
    }
}

fn intersect(world: &World, point: Position) -> Vec<Handle<PointerCollider>> {
    let mut result = Vec::with_capacity(8);
    world.foreach_fetch::<PointerCollider>(|handle, collider| {
        if collider.enabled && point.within(collider.rect) {
            result.push((handle, collider.order));
        }
    });

    result.sort_by(|(_, a), (_, b)| b.cmp(a));
    result.iter().map(|x| x.0).collect::<Vec<_>>()
}
