use crate::{
    elements::Element,
    world::{ElementHandle, ElementRemoved, World, WorldQueue},
};

#[derive(Default)]
pub struct IntersectManager;
impl Element for IntersectManager {}
impl IntersectManager {
    pub fn intersect(&self, world: &World) -> Option<ElementHandle> {
        let mut top_result = None;
        let max_order = isize::MIN;
        for intersection in world.elements::<Intersection>() {
            if intersection.z_order > max_order {
                top_result = Some(intersection.host);
            }
        }
        top_result
    }
}

pub struct Intersection {
    host: ElementHandle,
    rect: [i32; 4],
    z_order: isize,
}
impl Element for Intersection {
    fn when_inserted(&mut self, handle: ElementHandle, queue: &mut WorldQueue) {
        queue.queue(move |world| {
            let mut this = world.entry::<Intersection>(handle).unwrap();
            this.observe::<ElementRemoved>(|this, queue| {
                todo!("remove along with the host");
            });
        });
    }
}
impl Intersection {
    pub fn new(host: ElementHandle, rect: [i32; 4], z_order: isize) -> Self {
        Self {
            host,
            rect,
            z_order,
        }
    }
}
