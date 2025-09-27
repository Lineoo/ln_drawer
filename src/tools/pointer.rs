use hashbrown::HashMap;

use crate::{
    elements::{Element, Intersect, intersect::Collider},
    lnwin::PointerEvent,
    measures::Rectangle,
    tools::focus::{Focus, Focusable},
    world::{ElementHandle, ElementInserted, ElementRemoved, ElementUpdate, WorldCell},
};

#[derive(Default)]
pub struct PointerHitter {
    fallback: Option<ElementHandle>,
    hosts: HashMap<ElementHandle, ElementHandle>,
}
impl Element for PointerHitter {
    fn when_inserted(&mut self, _handle: ElementHandle, world: &WorldCell) {
        let mut pressed = false;
        let mut pointer_on = None;
        world.observe::<PointerEvent>(move |&event, world| {
            let intersect = world.single::<Intersect>().unwrap();
            let selection = world.single::<PointerHitter>().unwrap();

            let (PointerEvent::Moved(point)
            | PointerEvent::Pressed(point)
            | PointerEvent::Released(point)) = event;

            if !pressed {
                let collider = intersect.intersect(world, point);
                pointer_on = collider.and_then(|c| selection.hosts.get(&c).cloned());
            }

            if let PointerEvent::Pressed(_) = event {
                pressed = true;
                if let Some(mut focus) = world.single_mut::<Focus>() {
                    if let Some(pointer_on) = pointer_on
                        && world.contains_type::<dyn Focusable>(pointer_on)
                    {
                        focus.set(Some(pointer_on), world);
                    } else {
                        focus.set(None, world);
                    }
                }
            }

            if pressed {
                if let Some(mut pointer_on) = pointer_on.and_then(|w| world.entry(w)) {
                    pointer_on.trigger(PointerHit(event));
                } else if let Some(mut fallback) = selection.fallback.and_then(|w| world.entry(w)) {
                    fallback.trigger(PointerHit(event));
                }
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
            }
        });

        world.observe(|&ElementInserted(handle), world| {
            if let Some(hittable) = world.fetch::<dyn PointerHittable>(handle) {
                let collider = world.insert(Collider {
                    rect: hittable.get_hitting_rect(),
                    z_order: hittable.get_hitting_order(),
                });

                let mut hitter = world.single_mut::<PointerHitter>().unwrap();
                hitter.hosts.insert(collider, handle);

                (world.entry(handle).unwrap()).observe(move |ElementUpdate, world| {
                    let hittable = world.fetch::<dyn PointerHittable>(handle).unwrap();
                    let mut collider = world.fetch_mut::<Collider>(collider).unwrap();
                    collider.rect = hittable.get_hitting_rect();
                    collider.z_order = hittable.get_hitting_order();
                });

                (world.entry(handle).unwrap()).observe(move |ElementRemoved, world| {
                    let mut selection = world.single_mut::<PointerHitter>().unwrap();
                    selection.hosts.remove(&collider);
                    world.remove(collider);
                });
            }
        });
    }
}
impl PointerHitter {
    pub fn set_fallback(&mut self, element: ElementHandle) {
        self.fallback = Some(element);
    }
}

pub struct PointerHit(pub PointerEvent);
pub trait PointerHittable: Element {
    fn get_hitting_rect(&self) -> Rectangle;
    fn get_hitting_order(&self) -> isize;
}

pub trait PointerHitExt: PointerHittable + Sized {
    fn register_hittable(&mut self, handle: ElementHandle, world: &WorldCell);
}
impl<T: PointerHittable> PointerHitExt for T {
    fn register_hittable(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.register::<dyn PointerHittable>(|this| this.downcast_ref::<Self>().unwrap());
        this.register_mut::<dyn PointerHittable>(|this| this.downcast_mut::<Self>().unwrap());
    }
}
