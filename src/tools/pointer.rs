use crate::{
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    world::{Element, ElementHandle, WorldCell, WorldCellEntry},
};

pub struct PointerCollider {
    pub rect: Rectangle,
    pub z_order: isize,
}

pub struct PointerEnter;
pub struct PointerLeave;

pub struct PointerHit(pub PointerEvent);

#[derive(Default)]
pub struct Pointer {
    fallback: Option<ElementHandle>,
}
impl Element for Pointer {
    fn when_inserted(&mut self, entry: WorldCellEntry) {
        let mut pressed = false;
        let mut pointer_on = None;
        entry.world().observe::<PointerEvent>(move |&event, world| {
            let pointer = world.single::<Pointer>().unwrap();

            let (PointerEvent::Moved(point)
            | PointerEvent::Pressed(point)
            | PointerEvent::Released(point)) = event;

            if !pressed {
                // let pointer_onto = pointer.intersect(world, point);
                // if pointer_on != pointer_onto {
                //     if let Some(pointer_on) = pointer_on {
                //         world.entry(pointer_on).unwrap().trigger(PointerLeave);
                //     }
                //     if let Some(pointer_onto) = pointer_onto {
                //         world.entry(pointer_onto).unwrap().trigger(PointerEnter);
                //     }
                // }
                pointer_on = pointer.intersect(world, point);
            }

            if let PointerEvent::Pressed(_) = event {
                pressed = true;
                // FIXME This should be maintained by the element itself
                // if let Some(mut focus) = world.single_mut::<Focus>() {
                //     if let Some(pointer_on) = pointer_on
                //         && let Some(focusable) = world.fetch::<dyn Focusable>(pointer_on)
                //         && focusable.is_focusable()
                //     {
                //         focus.set(Some(pointer_on), world);
                //     } else {
                //         focus.set(None, world);
                //     }
                // }
            }

            if pressed {
                if let Some(mut pointer_on) = pointer_on.and_then(|w| world.entry(w)) {
                    pointer_on.trigger(PointerHit(event));
                } else if let Some(mut fallback) = pointer.fallback.and_then(|w| world.entry(w)) {
                    fallback.trigger(PointerHit(event));
                }
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
            }
        });
    }
}
impl Pointer {
    pub fn intersect(&self, world: &WorldCell, point: Position) -> Option<ElementHandle> {
        let mut top_result = None;
        let mut max_order = isize::MIN;
        world.foreach::<PointerCollider>(|intersection, handle| {
            if (intersection.z_order > max_order) && intersection.rect.contains(point) {
                max_order = intersection.z_order;
                top_result = Some(handle);
            }
        });
        top_result
    }

    pub fn get_fallback(&self) -> Option<ElementHandle> {
        self.fallback
    }

    pub fn set_fallback(&mut self, element: ElementHandle) {
        self.fallback = Some(element);
    }
}
