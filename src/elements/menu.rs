use crate::{
    elements::{ButtonRaw, Element, Label, PositionedElement},
    interface::{Interface, Square},
    measures::{Delta, Position, Rectangle},
    world::{ElementHandle, WorldCell},
};

pub struct Menu {
    frame: Square,
}
impl Element for Menu {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let rect1 = Rectangle::from_points(
            self.get_position() + Delta::new(10, 10),
            Rectangle::from_array(self.frame.get_rect()).right_up() - Delta::new(10, 10),
        );
        let button1 = world.insert(ButtonRaw::new(rect1, move |world| {
            world.insert(Label::new(rect1, "New Label".into(), world));
        }));
        let button1_text = world.insert(Label::new(rect1, "Label".into(), world));
        world.entry(button1).unwrap().depend(handle);
        world.entry(button1_text).unwrap().depend(handle);
    }
}
impl PositionedElement for Menu {
    fn get_position(&self) -> Position {
        Position::from_array(self.frame.get_position())
    }

    fn set_position(&mut self, position: Position) {
        self.frame.set_position(position.into_array());
    }
}
impl Menu {
    pub fn new(position: Position, interface: &mut Interface) -> Menu {
        let rect = Rectangle {
            origin: position,
            extend: Delta::new(200, 40),
        };
        let frame = interface.create_square(rect.into_array(), [0.1, 0.1, 0.1, 1.0]);

        Menu { frame }
    }
}
