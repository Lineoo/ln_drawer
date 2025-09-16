use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    elements::{Element, PositionChanged, PositionedElement},
    lnwin::PointerEvent,
    world::{ElementHandle, WorldCell},
};

pub struct Intersection {
    pub host: ElementHandle,
    pub rect: [i32; 4],
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
        let mut pointer_start = [0, 0];
        let mut element_start = [0, 0];
        this.observe::<PointerEvent>(move |&event, world| {
            let this = world.fetch::<IntersectManager>(handle).unwrap();
            if let PointerEvent::Pressed(point) = event {
                pressed = true;
                pointer_on = this.intersect(world, point);
            }

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
                        let delta = [point[0] - pointer_start[0], point[1] - pointer_start[1]];
                        let position = [element_start[0] + delta[0], element_start[1] + delta[1]];
                        positioned.set_position(position);
                        let mut entry = world.entry(pointer_on).unwrap();
                        entry.trigger(PositionChanged);
                    }
                }
            } else if let Some(pointer_on) = pointer_on {
                let mut pointer_on = world.entry(pointer_on).unwrap();
                pointer_on.trigger(IntersectHit(event));
            } else if pressed {
                world.trigger(IntersectFail(event));
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
                pointer_on = None;
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
    pub fn intersect(&self, world: &WorldCell, point: [i32; 2]) -> Option<ElementHandle> {
        let mut top_result = None;
        let max_order = isize::MIN;
        world.foreach::<Intersection>(|intersection| {
            if (intersection.z_order > max_order)
                && (intersection.rect[0] <= point[0] && point[0] < intersection.rect[2])
                && (intersection.rect[1] <= point[1] && point[1] < intersection.rect[3])
            {
                top_result = Some(intersection.host);
            }
        });
        top_result
    }
}

pub struct IntersectHit(pub PointerEvent);
pub struct IntersectFail(pub PointerEvent);
