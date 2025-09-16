use crate::{
    elements::{Element, ElementExt, PositionedElement, intersect::Intersection},
    interface::{Interface, Text},
    world::{ElementHandle, World, WorldCell},
};

pub struct Label {
    text: String,
    inner: Text,
}
impl Element for Label {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let intersect = world.insert(Intersection {
            host: handle,
            rect: self.inner.get_rect(),
            z_order: 0,
        });
        world.entry(intersect).unwrap().depend(handle);

        self.register::<dyn PositionedElement>(handle, world);
    }
}
impl PositionedElement for Label {
    fn get_position(&self) -> [i32; 2] {
        self.inner.get_position()
    }

    fn set_position(&mut self, position: [i32; 2]) {
        self.inner.set_position(position);
    }
}
impl Label {
    pub fn new(rect: [i32; 4], text: String, world: &mut World) -> Label {
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_text(rect, &text);
        Label { text, inner }
    }
}
