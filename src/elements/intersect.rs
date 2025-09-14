use crate::{
    elements::Element,
    lnwin::PointerEvent,
    world::{ElementHandle, ElementRemoved, WorldCell},
};

pub struct Intersection {
    pub host: ElementHandle,
    pub rect: [i32; 4],
    pub z_order: isize,
}

#[derive(Default)]
pub struct IntersectManager {
    boxes: Vec<Intersection>,
}
impl Element for IntersectManager {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry_dyn(handle).unwrap();
        let mut pressed = false;
        let mut pointer_on = None;
        this.observe::<PointerEvent>(move |&event, world| {
            let this = world.fetch::<IntersectManager>(handle).unwrap();
            if let PointerEvent::Pressed(point) = event {
                pressed = true;
                pointer_on = this.intersect(point)
            }

            if let Some(pointer_on) = pointer_on {
                let mut pointer_on = world.entry_dyn(pointer_on).unwrap();
                pointer_on.trigger(IntersectHit(event));
            } else if pressed {
                world.trigger(IntersectFail(event));
            }

            if let PointerEvent::Released(_) = event {
                pressed = false;
                pointer_on = None;
            }
        });
        this.observe::<ElementRemoved>(|this, queue| {
            todo!("remove along with the host");
        });
    }
}
impl IntersectManager {
    pub fn register(&mut self, intersection: Intersection) {
        self.boxes.push(intersection);
    }

    pub fn intersect(&self, point: [i32; 2]) -> Option<ElementHandle> {
        let mut top_result = None;
        let max_order = isize::MIN;
        for intersection in &self.boxes {
            if (intersection.z_order > max_order)
                && (intersection.rect[0] <= point[0] && point[0] < intersection.rect[2])
                && (intersection.rect[1] <= point[1] && point[1] < intersection.rect[3])
            {
                top_result = Some(intersection.host);
            }
        }
        top_result
    }
}

pub struct IntersectHit(pub PointerEvent);
pub struct IntersectFail(pub PointerEvent);
