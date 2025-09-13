use crate::{
    elements::Element,
    interface::{Interface, Text},
    world::World,
};

pub struct Label {
    text: String,
    inner: Text,
}
impl Element for Label {}
impl Label {
    pub fn new(rect: [i32; 4], text: String, world: &mut World) -> Label {
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_text(rect, &text);
        Label { text, inner }
    }

    pub fn get_position(&self) -> [i32; 2] {
        self.inner.get_position()
    }

    pub fn set_position(&mut self, position: [i32; 2]) {
        self.inner.set_position(position);
    }
}
