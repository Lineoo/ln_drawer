use std::{error::Error, path::Path};

use crate::{
    elements::{
        Element, OrderElement, OrderElementExt, PositionElementExt, PositionedElement,
        intersect::Collider,
    },
    interface::{Interface, Painter},
    measures::{Position, Rectangle},
    world::{ElementHandle, ElementUpdate, WorldCell},
};

pub struct Image {
    painter: Painter,
}
impl Element for Image {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let intersect = world.insert(Collider {
            rect: Rectangle::from_array(self.painter.get_rect()),
            z_order: 0,
        });
        world.entry(intersect).unwrap().depend(handle);
        (world.entry(handle).unwrap()).observe::<ElementUpdate>(move |_event, world| {
            let position = world
                .fetch::<dyn PositionedElement>(handle)
                .unwrap()
                .get_position();
            let mut intersect = world.fetch_mut::<Collider>(intersect).unwrap();

            intersect.rect.origin = position;
        });

        self.register_position(handle, world);
        self.register_order(handle, world);
    }
}
impl PositionedElement for Image {
    fn get_position(&self) -> Position {
        Position::from_array(self.painter.get_position())
    }

    fn set_position(&mut self, position: Position) {
        self.painter.set_position(position.into_array());
    }
}
impl OrderElement for Image {
    fn get_order(&self) -> isize {
        self.painter.get_z_order()
    }

    fn set_order(&mut self, order: isize) {
        self.painter.set_z_order(order);
    }
}
impl Image {
    pub fn new(path: impl AsRef<Path>, interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let reader = image::ImageReader::open(path)?;
        let image = reader.decode()?;

        let painter = interface.create_painter_with(
            [0, 0, image.width() as i32, image.height() as i32],
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }

    pub fn from_bytes(bytes: &[u8], interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;

        let painter = interface.create_painter_with(
            [0, 0, image.width() as i32, image.height() as i32],
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }
}
