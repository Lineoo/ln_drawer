use crate::{
    lnwin::{Lnwindow, PointerEvent},
    measures::{Position, Rectangle, ZOrder},
    tools::focus::Focus,
    world::{Element, ElementHandle, WorldCell, WorldCellEntry},
};

#[derive(Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub z_order: ZOrder,
}

pub struct PointerEnter;
pub struct PointerLeave;

#[derive(Clone, Copy)]
pub struct PointerHit(pub PointerEvent);

impl Element for PointerCollider {}

#[derive(Default)]
pub struct Pointer;
impl Element for Pointer {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        let mut pressed = false;
        let mut pointer_on = None;
        entry
            .single_other::<Lnwindow>()
            .unwrap()
            .observe::<PointerEvent>(move |&event, entry| {
                let pointer = entry.fetch().unwrap();

                let (PointerEvent::Moved(point)
                | PointerEvent::Pressed(point)
                | PointerEvent::Released(point)) = event;

                if !pressed {
                    let pointer_onto = pointer.intersect(entry.world(), point);
                    if pointer_on != pointer_onto {
                        if let Some(pointer_on) = pointer_on.and_then(|e| entry.entry(e)) {
                            pointer_on.trigger(PointerLeave);
                        }
                        if let Some(pointer_onto) = pointer_onto.and_then(|e| entry.entry(e)) {
                            pointer_onto.trigger(PointerEnter);
                        }
                    }
                    pointer_on = pointer.intersect(entry.world(), point);
                }

                if let PointerEvent::Pressed(_) = event {
                    pressed = true;
                    let mut focus = entry.single_fetch_mut::<Focus>().unwrap();
                    focus.set(None, &entry);
                }

                if pressed && let Some(pointer_on) = pointer_on.and_then(|w| entry.entry(w)) {
                    pointer_on.trigger(PointerHit(event));
                }

                if let PointerEvent::Released(_) = event {
                    pressed = false;
                }
            });
    }
}
impl Pointer {
    pub fn intersect(
        &self,
        world: &WorldCell,
        point: Position,
    ) -> Option<ElementHandle<PointerCollider>> {
        let mut top_result = None;
        let mut max_order = ZOrder::new(isize::MIN);
        world.foreach_fetch::<PointerCollider>(|handle, intersection| {
            if (intersection.z_order > max_order) && intersection.rect.contains(point) {
                max_order = intersection.z_order;
                top_result = Some(handle);
            }
        });

        top_result
    }
}
