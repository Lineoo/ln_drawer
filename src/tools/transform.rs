use crate::{
    interface::{Interface, Wireframe},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    tools::pointer::{Pointer, PointerHit},
    world::{Element, ElementHandle, InsertElement, WorldCellEntry},
};

#[derive(Default)]
pub struct TransformTool {
    selected: Option<Selected>,
    dragging: Option<Dragging>,
}

struct Selected {
    frame: Wireframe,
    element: ElementHandle,
}

struct Dragging {
    element_start: Position,
    pointer_start: Position,
}

impl Element for TransformTool {}
impl InsertElement for TransformTool {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe(|PointerHit(event), entry| {
            let this = &mut *entry.fetch_mut().unwrap();

            match (&mut this.selected, &mut this.dragging, event) {
                (_, None, PointerEvent::Pressed(pointer)) => {
                    let pointer_tool = entry.single_fetch::<Pointer>().unwrap();
                    if let Some(element) = pointer_tool.intersect(&entry, *pointer)
                        && let Some(rect) = entry.get::<Rectangle>(element)
                    {
                        let mut interact = entry.single_fetch_mut::<Interface>().unwrap();
                        let frame = interact.create_wireframe(rect, [0.8, 0.8, 0.8, 0.9]);
                        this.selected = Some(Selected { frame, element });
                        this.dragging = Some(Dragging {
                            element_start: rect.origin,
                            pointer_start: *pointer,
                        });
                    } else {
                        this.selected = None;
                        this.dragging = None;
                    }
                }
                (
                    Some(Selected { frame, element }),
                    Some(Dragging {
                        element_start,
                        pointer_start,
                    }),
                    PointerEvent::Moved(pointer),
                ) => {
                    let delta = *pointer - *pointer_start;
                    let dest = *element_start + delta;

                    if let Some(mut rect) = entry.get::<Rectangle>(*element) {
                        rect.origin = dest;
                        entry.set::<Rectangle>(*element, rect);
                        frame.set_rect(rect);
                    }
                }
                (
                    Some(Selected { frame, element }),
                    Some(Dragging {
                        element_start,
                        pointer_start,
                    }),
                    PointerEvent::Released(pointer),
                ) => {
                    let delta = *pointer - *pointer_start;
                    let dest = *element_start + delta;

                    if let Some(mut rect) = entry.get::<Rectangle>(*element) {
                        rect.origin = dest;
                        entry.set::<Rectangle>(*element, rect);
                        frame.set_rect(rect);
                    }

                    this.dragging = None;
                }
                _ => {}
            }
        });
    }
}
