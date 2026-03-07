#![allow(deprecated)]

use std::cell::Cell;

use winit::event::{
    ButtonSource, ElementState, MouseButton, PointerKind, PointerSource, WindowEvent,
};

use crate::{
    layout::LayoutRectangle,
    lnwin::Lnwindow,
    measures::{Position, Rectangle, Size},
    render::viewport::Viewport,
    tools::{mouse::MouseTool, viewport::ViewportUtils},
    world::{Element, Handle, World, WorldError},
};

/// Guaranteed for single-pointer operations like mouse cursor or the first-touch finger.
///
/// **Associated Events**: [`PointerHit`], [`PointerHover`], [`PointerMenu`]
#[derive(Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHit {
    pub position: Position,
    pub screen: [f64; 2],
    pub status: PointerHitStatus,
    pub data: PointerHitData,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHitStatus {
    Press,
    Moving,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHitData {
    pub force: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHover {
    pub position: Position,
    pub screen: [f64; 2],
    pub status: PointerHoverStatus,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHoverStatus {
    Enter,
    Moving,
    Leave,
}

struct PointerTool {
    /// the main pointer that takes effect
    pointer: Option<Pointer>,
}

struct Pointer {
    position: Position,
    screen: [f64; 2],
    kind: PointerKind,
    hovering: Option<Handle<PointerCollider>>,
    pressed: Option<Press>,
}

#[derive(Clone, Copy)]
struct Press {
    force: Option<f32>,
}

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

    pub fn intersect(world: &World, point: Position) -> Vec<Handle<PointerCollider>> {
        let mut result = Vec::with_capacity(8);
        world.foreach_fetch::<PointerCollider>(|collider| {
            if collider.enabled && point.within(collider.rect) {
                result.push((collider.handle(), collider.order));
            }
        });

        result.sort_by(|(_, a), (_, b)| b.cmp(a));
        result.iter().map(|x| x.0).collect::<Vec<_>>()
    }
}

impl Element for PointerCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        if let Err(WorldError::SingletonNoSuch(_)) = world.single::<PointerTool>() {
            world.insert(PointerTool { pointer: None });
        }

        if let Err(WorldError::SingletonNoSuch(_)) = world.single::<MouseTool>() {
            world.insert(MouseTool::default());
        }

        world.observer(this, move |&LayoutRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.queue(|world| {
            let mut tool = world.single_fetch_mut::<PointerTool>().unwrap();
            if let Some(pointer) = &mut tool.pointer {
                pointer.recalculate_hovering(world);
            }
        });
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        world.queue(|world| {
            let mut tool = world.single_fetch_mut::<PointerTool>().unwrap();
            if let Some(pointer) = &mut tool.pointer {
                pointer.recalculate_hovering(world);
            }
        });
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        world.queue(|world| {
            let mut tool = world.single_fetch_mut::<PointerTool>().unwrap();
            if let Some(pointer) = &mut tool.pointer {
                pointer.recalculate_hovering(world);
            }
        });
    }
}

impl PointerTool {
    fn alloc_pointer(&mut self, kind: PointerKind) -> Option<&mut Pointer> {
        if self.pointer.is_none() {
            self.pointer = Some(Pointer {
                position: Position::default(),
                screen: Default::default(),
                kind,
                hovering: None,
                pressed: None,
            });

            self.pointer.as_mut()
        } else if let Some(pointer) = &self.pointer
            && pointer.kind == kind
        {
            self.pointer.as_mut()
        } else {
            None
        }
    }
}

impl Element for PointerTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            let viewport = world.single_fetch::<Viewport>().unwrap();

            match event {
                WindowEvent::PointerMoved {
                    position, source, ..
                } => {
                    let kind = PointerKind::from(source.clone());

                    let Some(pointer) = this.alloc_pointer(kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    let position = viewport.screen_to_world_absolute(screen).floor();

                    drop((lnwindow, viewport));
                    if let Some(press) = &mut pointer.pressed {
                        press.force = match source {
                            PointerSource::Mouse => Some(1.0),
                            PointerSource::Touch { force, .. } => {
                                force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::TabletTool { data, .. } => {
                                data.force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::Unknown => None,
                        };
                    }
                    pointer.update_position(world, position, screen);
                }

                WindowEvent::PointerButton {
                    position,
                    button,
                    state,
                    ..
                } => {
                    let kind = match button {
                        ButtonSource::Mouse(MouseButton::Left) => PointerKind::Mouse,
                        ButtonSource::Mouse(_) => return,
                        ButtonSource::Touch { finger_id, .. } => PointerKind::Touch(*finger_id),
                        ButtonSource::TabletTool { kind, .. } => PointerKind::TabletTool(*kind),
                        ButtonSource::Unknown(_) => PointerKind::Unknown,
                    };

                    let Some(pointer) = this.alloc_pointer(kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    let position = viewport.screen_to_world_absolute(screen).floor();

                    drop((lnwindow, viewport));
                    pointer.update_position(world, position, screen);
                    pointer.update_pressed(
                        world,
                        match state {
                            ElementState::Pressed => Some(Press {
                                force: match button {
                                    ButtonSource::Mouse(_) => Some(1.0),
                                    ButtonSource::Touch { force, .. } => {
                                        force.map(|x| x.normalized(None) as f32)
                                    }
                                    ButtonSource::TabletTool { data, .. } => {
                                        data.force.map(|x| x.normalized(None) as f32)
                                    }
                                    ButtonSource::Unknown(_) => None,
                                },
                            }),
                            ElementState::Released => None,
                        },
                    );
                }

                WindowEvent::PointerEntered { position, kind, .. } => {
                    let Some(pointer) = this.alloc_pointer(*kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    let position = viewport.screen_to_world_absolute(screen).floor();

                    drop((lnwindow, viewport));
                    pointer.update_position(world, position, screen);
                }

                WindowEvent::PointerLeft { position, kind, .. } => {
                    let Some(pointer) = this.alloc_pointer(*kind) else {
                        return;
                    };

                    if let Some(position) = *position {
                        let screen = lnwindow.cursor_to_screen(position);
                        let position = viewport.screen_to_world_absolute(screen).floor();

                        drop((lnwindow, viewport));
                        pointer.update_position(world, position, screen);
                    } else {
                        drop((lnwindow, viewport));
                    }

                    if let Some(hovering) = pointer.hovering {
                        world.trigger(
                            hovering,
                            &PointerHover {
                                position: pointer.position,
                                screen: pointer.screen,
                                status: PointerHoverStatus::Leave,
                                pointer: PointerKind::Mouse,
                            },
                        );
                    }

                    this.pointer = None;
                }

                _ => {}
            }
        });
    }
}

impl Pointer {
    fn update_position(&mut self, world: &World, position: Position, screen: [f64; 2]) {
        self.position = position;
        self.screen = screen;

        self.recalculate_hovering(world);

        let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();
        viewport_utils.cursor(world, screen);
        drop(viewport_utils);

        if let Some(hovering) = self.hovering {
            if let Some(pressed) = self.pressed {
                world.trigger(
                    hovering,
                    &PointerHit {
                        position,
                        screen,
                        status: PointerHitStatus::Moving,
                        data: PointerHitData {
                            force: pressed.force,
                        },
                        pointer: self.kind,
                    },
                );
            }

            world.trigger(
                hovering,
                &PointerHover {
                    position,
                    screen,
                    status: PointerHoverStatus::Moving,
                    pointer: self.kind,
                },
            );
        }
    }

    fn update_pressed(&mut self, world: &World, pressed: Option<Press>) {
        if let Some(hovering) = self.hovering {
            let hit = match (self.pressed, pressed) {
                (None, Some(press)) => Some(PointerHit {
                    position: self.position,
                    screen: self.screen,
                    status: PointerHitStatus::Press,
                    data: PointerHitData { force: press.force },
                    pointer: self.kind,
                }),
                (Some(press), None) => Some(PointerHit {
                    position: self.position,
                    screen: self.screen,
                    status: PointerHitStatus::Release,
                    data: PointerHitData { force: press.force },
                    pointer: self.kind,
                }),
                _ => None,
            };

            if let Some(hit) = hit {
                world.trigger(hovering, &hit);
            }
        }

        self.pressed = pressed;
        self.recalculate_hovering(world);
    }

    fn update_hovering(&mut self, world: &World, hovering: Option<Handle<PointerCollider>>) {
        let previous = self.hovering;
        self.hovering = hovering;

        if let Some(previous) = previous {
            world.trigger(
                previous,
                &PointerHover {
                    position: self.position,
                    screen: self.screen,
                    status: PointerHoverStatus::Leave,
                    pointer: self.kind,
                },
            );
        }

        if let Some(hovering) = hovering {
            world.trigger(
                hovering,
                &PointerHover {
                    position: self.position,
                    screen: self.screen,
                    status: PointerHoverStatus::Enter,
                    pointer: self.kind,
                },
            );
        }
    }

    fn recalculate_hovering(&mut self, world: &World) {
        if self.pressed.is_some() {
            return;
        }

        let mut landing = None;
        for each in PointerCollider::intersect(world, self.position) {
            let check = PointerCheck {
                position: self.position,
                occlude: Cell::new(true),
            };
            world.trigger(each, &check);
            if check.occlude.get() {
                landing = Some(each);
                break;
            }
        }

        if self.hovering != landing {
            self.update_hovering(world, landing);
        }
    }
}

// [deprecated] //

/// Similar to [`PointerCollider`], but will react when mouse hover on
/// its edge and provide detailed information on which edge it hit.
///
/// **Event associated**: [`PointerHitEdge`]
#[derive(Debug, Clone, Copy)]
#[deprecated]
pub struct PointerEdgeCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy)]
#[deprecated]
pub struct PointerHitEdge {
    pub position: Position,
    pub status: PointerHitStatus,
    pub edge: PointerEdge,
}

#[derive(Debug, Clone, Copy)]
#[deprecated]
pub struct PointerHoverEdge {
    pub position: Position,
    pub motion: PointerHoverStatus,
    pub edge: PointerEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[deprecated]
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

#[deprecated]
struct ColliderUpdate;
#[deprecated]
struct ColliderEdgeLock {
    edge: Option<PointerEdge>,
}

#[derive(Debug)]
#[deprecated]
pub struct PointerCheck {
    pub position: Position,
    pub occlude: Cell<bool>,
}

#[derive(Debug)]
#[deprecated]
pub struct PointerEdgeCheck {
    pub position: Position,
    pub edge: PointerEdge,
    pub occlude: Cell<bool>,
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

        world.observer(collider, move |event: &PointerCheck, world| {
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

        world.observer(collider, move |event: &PointerHit, world| {
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

        world.observer(this, move |ColliderUpdate, world| {
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

impl Element for ColliderEdgeLock {}
