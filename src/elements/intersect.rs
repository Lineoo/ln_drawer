use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    elements::{Element, PositionChanged, PositionedElement},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    world::{ElementHandle, WorldCell},
};

pub struct Intersection {
    pub host: ElementHandle,
    pub rect: Rectangle,
    pub z_order: isize,
}
impl Element for Intersection {}

#[derive(Default)]
pub struct IntersectManager {
    dragging: bool,
}
impl Element for IntersectManager {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        let mut pressed = false;
        let mut pointer_on = None;
        // Dragging
        let mut pointer_start = Position::default();
        let mut element_start = Position::default();
        this.observe::<PointerEvent>(move |&event, world| {
            let this = world.fetch::<IntersectManager>(handle).unwrap();

            let (PointerEvent::Moved(point)
            | PointerEvent::Pressed(point)
            | PointerEvent::Released(point)) = event;

            if !pressed {
                let pointer_onto = this.intersect(world, point);
                if pointer_on != pointer_onto {
                    if let Some(pointer_on) = pointer_on {
                        world.entry(pointer_on).unwrap().trigger(PointerLeave);
                    }
                    if let Some(pointer_onto) = pointer_onto {
                        world.entry(pointer_onto).unwrap().trigger(PointerEnter);
                    }
                }
                pointer_on = pointer_onto;
            }

            if pointer_on.is_some_and(|it| !world.contains(it)) {
                pointer_on = None;
            }

            if let Some(pointer_on) = pointer_on {
                world.entry(pointer_on).unwrap().trigger(PointerHover(point));
            }

            if let PointerEvent::Pressed(_) = event {
                pressed = true;
            }

            if pressed {
                if this.dragging {
                    if let Some(pointer_on) = pointer_on
                        && let Some(mut positioned) =
                            world.fetch_mut::<dyn PositionedElement>(pointer_on)
                    {
                        if let PointerEvent::Pressed(point) = event {
                            element_start = positioned.get_position();
                            pointer_start = point;
                        }
                        if let PointerEvent::Moved(point) = event {
                            let delta = point - pointer_start;
                            let position = element_start + delta;
                            positioned.set_position(position);
                            let mut entry = world.entry(pointer_on).unwrap();
                            entry.trigger(PositionChanged);
                        }
                    }
                } else if let Some(pointer_on) = pointer_on {
                    let mut pointer_on = world.entry(pointer_on).unwrap();
                    pointer_on.trigger(IntersectHit(event));
                } else {
                    world.trigger(IntersectFail(event));
                }
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
            }
        });
        this.observe::<WindowEvent>(move |event, world| {
            if let WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyS),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } = event
            {
                let mut this = world.fetch_mut::<IntersectManager>(handle).unwrap();
                this.dragging = match state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                }
            }
        });
    }
}
impl IntersectManager {
    pub fn intersect(&self, world: &WorldCell, point: Position) -> Option<ElementHandle> {
        let mut top_result = None;
        let mut max_order = isize::MIN;
        world.foreach::<Intersection>(|intersection| {
            if (intersection.z_order > max_order) && intersection.rect.contains(point) {
                max_order = intersection.z_order;
                top_result = Some(intersection.host);
            }
        });
        top_result
    }
}

pub struct PointerEnter;
pub struct PointerHover(pub Position);
pub struct PointerLeave;

pub struct IntersectHit(pub PointerEvent);
pub struct IntersectFail(pub PointerEvent);
