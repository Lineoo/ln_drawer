use winit::event::{ElementState, MouseButton, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::{Position, PositionFract, Rectangle, Size},
    render::viewport::Viewport,
    world::{Element, Handle, World},
};

#[derive(Debug, Clone, Copy)]
pub struct PointerCollider {
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

impl Element for PointerCollider {}

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

#[derive(Debug, Default)]
pub struct PointerTool {
    position: PositionFract,
    hovering: Option<Handle<PointerCollider>>,
    pressed: bool,
}

impl Element for PointerTool {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
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
                            let landing = pointer.intersect(world, position.floor());

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
                        if let Some(target) = pointer.intersect(world, pointer.position.floor()) {
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

        world.observer(this, |event: &PointerHit, world, pointer| {
            let (PointerHit::Moving(position)
            | PointerHit::Pressed(position)
            | PointerHit::Released(position)) = *event;

            let pointer = world.fetch(pointer).unwrap();
            if let Some(hovering) = pointer.intersect(world, position) {
                world.trigger(hovering, *event);
            }
        });

        world.observer(this, |event: &PointerMenu, world, pointer| {
            let PointerMenu(position) = *event;

            let pointer = world.fetch(pointer).unwrap();
            if let Some(hovering) = pointer.intersect(world, position) {
                world.trigger(hovering, *event);
            }
        });
    }
}
impl PointerTool {
    pub fn intersect(&self, world: &World, point: Position) -> Option<Handle<PointerCollider>> {
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
}
