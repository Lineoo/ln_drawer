use crate::{
    lnwin::{Lnwindow, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::focus::{Focus, RequestFocus},
    world::{Element, Handle, World},
};

#[derive(Debug, Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub z_order: ZOrder,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHit(pub PointerEvent);

#[derive(Debug, Clone, Copy)]
pub struct PointerEnter;

#[derive(Debug, Clone, Copy)]
pub struct PointerLeave;

impl Element for PointerCollider {}

impl PointerCollider {
    pub fn fullscreen(z_order: ZOrder) -> PointerCollider {
        PointerCollider {
            rect: Rectangle {
                origin: Position::splat(i32::MIN / 2),
                extend: Delta::splat(i32::MAX),
            },
            z_order,
        }
    }
}

#[derive(Default)]
pub struct Pointer;
impl Element for Pointer {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let mut pressed = false;
        let mut pointer_on = None;

        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |&event, world, _| {
            let pointer = world.fetch(this).unwrap();

            let (PointerEvent::Moved(point)
            | PointerEvent::Pressed(point)
            | PointerEvent::Released(point)) = event;

            if !pressed {
                let pointer_onto = pointer.intersect(world, point);
                if pointer_on != pointer_onto {
                    if let Some(pointer_on) = pointer_on {
                        world.trigger(pointer_on, PointerLeave);
                    }

                    if let Some(pointer_onto) = pointer_onto {
                        world.trigger(pointer_onto, PointerEnter);
                    }
                }

                pointer_on = pointer.intersect(world, point);
            }

            if let PointerEvent::Pressed(_) = event {
                pressed = true;
                let focus = world.single::<Focus>().unwrap();
                world.trigger(focus, RequestFocus(Some(this.untyped())));
            }

            if pressed && let Some(pointer_on) = pointer_on {
                world.trigger(pointer_on, PointerHit(event));
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
            }
        });
    }
}
impl Pointer {
    pub fn intersect(&self, world: &World, point: Position) -> Option<Handle<PointerCollider>> {
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
