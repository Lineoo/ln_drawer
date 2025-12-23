use winit::event::{ElementState, MouseButton, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::{Position, PositionFract, Rectangle, Size},
    render::viewport::Viewport,
    world::{Element, Handle, World},
};

/// See [`PointerColliderEdge`] for a more specific version for frame ops.
///
/// **Event associated**: [`PointerHit`], [`PointerMenu`], [`PointerEnter`], [`PointerLeave`]
#[derive(Debug, Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub order: isize,
}

/// Similar to [`PointerCollider`], but will react when mouse hover on
/// its edge and provide detailed information on which edge it hit.
///
/// **Event associated**: [`PointerHitEdge`]
#[derive(Debug, Clone, Copy)]
pub struct PointerEdgeCollider {
    pub rect: Rectangle,
    pub order: isize,
}

#[derive(Debug, Clone, Copy)]
pub enum PointerHit {
    Pressed(Position),
    Moving(Position),
    Released(Position),
}

#[derive(Debug, Clone, Copy)]
pub struct PointerMenu(pub Position);

#[derive(Debug, Clone, Copy)]
pub struct PointerEnter;

#[derive(Debug, Clone, Copy)]
pub struct PointerLeave;

#[derive(Debug, Clone, Copy)]
pub struct PointerHitEdge {
    pub hit: PointerHit,
    pub edge: PointerEdge,
}

#[derive(Debug, Clone, Copy)]
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

struct ColliderUpdate;
struct ColliderEdgeLock {
    edge: Option<PointerEdge>,
}

impl PointerCollider {
    pub fn fullscreen(order: isize) -> PointerCollider {
        PointerCollider {
            rect: Rectangle {
                origin: Position::MIN,
                extend: Size::MAX,
            },
            order,
        }
    }
}

impl PointerHit {
    pub fn position(&self) -> Position {
        let (PointerHit::Pressed(position)
        | PointerHit::Moving(position)
        | PointerHit::Released(position)) = *self;
        position
    }
}

impl PointerHitEdge {
    #[inline]
    pub fn position(&self) -> Position {
        self.hit.position()
    }
}

impl Element for PointerCollider {}

impl Element for ColliderEdgeLock {}

impl Element for PointerEdgeCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        const EXPAND: i32 = 5;

        let collider = world.insert(PointerCollider {
            rect: self.rect.expand(EXPAND),
            order: self.order,
        });

        let lock = world.insert(ColliderEdgeLock { edge: None });

        world.observer(collider, move |event: &PointerHit, world, _| {
            let this = world.fetch(this).unwrap();
            let mut lock = world.fetch_mut(lock).unwrap();

            match (event, lock.edge) {
                (PointerHit::Pressed(position), None) => {
                    let mut idx = 0;

                    let shrink = this.rect.expand(-EXPAND);

                    if position.x < shrink.left() {
                        idx += 0;
                    } else if position.x < shrink.right() {
                        idx += 1;
                    } else {
                        idx += 2;
                    }

                    if position.y < shrink.down() {
                        idx += 0;
                    } else if position.y < shrink.up() {
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
                    world.trigger(this.handle(), PointerHitEdge { hit: *event, edge });
                }

                (PointerHit::Moving(_), Some(edge)) => {
                    world.trigger(this.handle(), PointerHitEdge { hit: *event, edge });
                }

                (PointerHit::Released(_), Some(edge)) => {
                    lock.edge = None;
                    world.trigger(this.handle(), PointerHitEdge { hit: *event, edge });
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
            world.trigger(this, ColliderUpdate);
        });
    }
}

#[derive(Debug, Default)]
pub struct PointerTool {
    position: PositionFract,
    hovering: Option<Handle<PointerCollider>>,
    pressed: bool,
}

impl Element for PointerTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            world.single::<Lnwindow>().unwrap(),
            move |event: &WindowEvent, world, lnwindow| {
                let mut pointer = world.fetch_mut(this).unwrap();
                let lnwindow = world.fetch(lnwindow).unwrap();
                lnwindow.request_redraw();

                match event {
                    WindowEvent::CursorMoved { position, .. } => {
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        let position = lnwindow.cursor_to_screen(*position);
                        let position = viewport.screen_to_world_absolute(position);

                        if !pointer.pressed {
                            // pointer transmit
                            let landing = intersect(world, position.floor());

                            if pointer.hovering != landing {
                                if let Some(hovering) = pointer.hovering {
                                    world.trigger(hovering, PointerLeave);
                                }

                                if let Some(landing) = landing {
                                    world.trigger(landing, PointerEnter);
                                }

                                pointer.hovering = landing;
                            }
                        } else if let Some(hovering) = pointer.hovering {
                            world.trigger(hovering, PointerHit::Moving(position.floor()));
                        }

                        pointer.position = position;
                    }

                    WindowEvent::MouseInput {
                        button: MouseButton::Left,
                        state: ElementState::Pressed,
                        ..
                    } => {
                        if let Some(hovering) = pointer.hovering {
                            world.trigger(hovering, PointerHit::Pressed(pointer.position.floor()));
                        }

                        pointer.pressed = true;
                    }

                    WindowEvent::MouseInput {
                        button: MouseButton::Left,
                        state: ElementState::Released,
                        ..
                    } => {
                        if let Some(hovering) = pointer.hovering {
                            world.trigger(hovering, PointerHit::Released(pointer.position.floor()));
                        }

                        pointer.pressed = false;
                    }

                    WindowEvent::MouseInput {
                        button: MouseButton::Right,
                        state: ElementState::Pressed,
                        ..
                    } => {
                        if let Some(target) = intersect(world, pointer.position.floor()) {
                            world.trigger(target, PointerMenu(pointer.position.floor()));
                        }
                    }

                    _ => {}
                }
            },
        );

        // reproduce events
        // personally not flavor, but no better idea tbh
        // may figure out how the time i got click-through

        world.observer(this, |event: &PointerHit, world, _| {
            if let Some(hovering) = intersect(world, event.position()) {
                world.trigger(hovering, *event);
            }
        });

        world.observer(this, |event: &PointerMenu, world, _| {
            let PointerMenu(position) = *event;

            if let Some(hovering) = intersect(world, position) {
                world.trigger(hovering, *event);
            }
        });
    }
}

fn intersect(world: &World, point: Position) -> Option<Handle<PointerCollider>> {
    let mut top_result = None;
    let mut max_order = isize::MIN;
    world.foreach_fetch::<PointerCollider>(|handle, intersection| {
        if (intersection.order > max_order) && point.within(intersection.rect) {
            max_order = intersection.order;
            top_result = Some(handle);
        }
    });

    top_result
}
