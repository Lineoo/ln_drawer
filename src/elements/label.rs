use crate::{
    elements::{
        tools::pointer::{PointerHitExt, PointerHittable}, Element, OrderElement, OrderElementExt, PositionElementExt, PositionedElement
    },
    interface::{Interface, Text},
    measures::{Position, Rectangle},
    world::{ElementHandle, WorldCell},
};

pub struct Label {
    text: String,
    inner: Text,
}
impl Element for Label {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        self.register_position(handle, world);
        self.register_order(handle, world);
        self.register_hittable(handle, world);
    }
}
impl PositionedElement for Label {
    fn get_position(&self) -> Position {
        Position::from_array(self.inner.get_position())
    }

    fn set_position(&mut self, position: Position) {
        self.inner.set_position(position.into_array());
    }
}
impl OrderElement for Label {
    fn get_order(&self) -> isize {
        self.inner.get_z_order()
    }

    fn set_order(&mut self, order: isize) {
        self.inner.set_z_order(order);
    }
}
impl PointerHittable for Label {
    fn get_hitting_rect(&self) -> Rectangle {
        Rectangle::from_array(self.inner.get_rect())
    }

    fn get_hitting_order(&self) -> isize {
        self.inner.get_z_order()
    }
}
impl Label {
    pub fn new(rect: Rectangle, text: String, world: &WorldCell) -> Label {
        let mut interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_text(rect.into_array(), &text);
        Label { text, inner }
    }
}
