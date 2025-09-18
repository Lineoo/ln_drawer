use crate::{
    elements::{
        Element, PositionChanged, PositionElementExt, PositionedElement, intersect::Intersection,
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
        let intersect = world.insert(Intersection {
            host: handle,
            rect: Rectangle::from_array(self.inner.get_rect()),
            z_order: 0,
        });
        world.entry(intersect).unwrap().depend(handle);
        (world.entry(handle).unwrap()).observe::<PositionChanged>(move |_event, world| {
            let position = world
                .fetch::<dyn PositionedElement>(handle)
                .unwrap()
                .get_position();
            let mut intersect = world.fetch_mut::<Intersection>(intersect).unwrap();

            intersect.rect.origin = position;
        });

        self.register_position(handle, world);
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
impl Label {
    pub fn new(rect: Rectangle, text: String, world: &WorldCell) -> Label {
        let mut interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_text(rect.into_array(), &text);
        Label { text, inner }
    }
}
