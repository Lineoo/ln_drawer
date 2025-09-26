use crate::{
    elements::Element,
    measures::{Position, Rectangle},
    world::{ElementHandle, WorldCell},
};

#[derive(Default)]
pub struct Intersect {}
impl Element for Intersect {}
impl Intersect {
    pub fn intersect(&self, world: &WorldCell, point: Position) -> Option<ElementHandle> {
        let mut top_result = None;
        let mut max_order = isize::MIN;
        world.foreach::<Collider>(|intersection, handle| {
            if (intersection.z_order > max_order) && intersection.rect.contains(point) {
                max_order = intersection.z_order;
                top_result = Some(handle);
            }
        });
        top_result
    }
}

pub struct Collider {
    pub rect: Rectangle,
    pub z_order: isize,
}
impl Element for Collider {}
