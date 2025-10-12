use crate::{
    lnwin::{Lnwindow, PointerEvent},
    measures::{Position, Rectangle, ZOrder},
    world::{Element, ElementHandle, WorldCell, WorldCellEntry},
};

#[derive(Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub z_order: ZOrder,
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
        entry
            .single_entry::<Lnwindow>()
            .unwrap()
            .observe::<PointerEvent>(move |&event, world| {
                let pointer = world.single_fetch::<Pointer>().unwrap();

                let (PointerEvent::Moved(point)
                | PointerEvent::Pressed(point)
                | PointerEvent::Released(point)) = event;

                if !pressed {
                    let pointer_onto = pointer.intersect(world.world(), point);
                    if pointer_on != pointer_onto {
                        if let Some(mut pointer_on) = pointer_on.and_then(|e| world.entry(e)) {
                            pointer_on.trigger(PointerLeave);
                        }
                        if let Some(mut pointer_onto) = pointer_onto.and_then(|e| world.entry(e)) {
                            pointer_onto.trigger(PointerEnter);
                        }
                    }
                    pointer_on = pointer.intersect(world.world(), point);
                }

                if let PointerEvent::Pressed(_) = event {
                    pressed = true;
                }

                if pressed {
                    if let Some(mut pointer_on) = pointer_on.and_then(|w| world.entry(w)) {
                        pointer_on.trigger(PointerHit(event));
                    } else if let Some(mut fallback) = pointer.fallback.and_then(|w| world.entry(w))
                    {
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
    // TODO Optimize
    pub fn intersect(&self, world: &WorldCell, point: Position) -> Option<ElementHandle> {
        let mut top_result = None;
        let mut max_order = ZOrder::new(isize::MIN);
        world.get_foreach::<PointerCollider>(|handle, intersection| {
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
