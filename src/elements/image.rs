use std::{error::Error, path::Path};

use crate::{
    elements::{Element, IntersectManager, PositionedElement, intersect::Intersection},
    interface::{Interface, Painter},
    world::{ElementHandle, World, WorldCell},
};

pub struct Image {
    painter: Painter,
}
impl Element for Image {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut intersect = world.single_mut::<IntersectManager>().unwrap();
        intersect.register(Intersection {
            host: handle,
            rect: self.painter.get_rect(),
            z_order: 0,
        });
    }

    fn as_positioned(&mut self) -> Option<&mut dyn PositionedElement> {
        Some(self)
    }
}
impl PositionedElement for Image {
    fn get_position(&self) -> [i32; 2] {
        self.painter.get_position()
    }

    fn set_position(&mut self, position: [i32; 2]) {
        self.painter.set_position(position);
    }
}
impl Image {
    pub fn new(path: impl AsRef<Path>, world: &mut World) -> Result<Image, Box<dyn Error>> {
        let reader = image::ImageReader::open(path)?;
        let image = reader.decode()?;

        let interface = world.single_mut::<Interface>().unwrap();
        let painter = interface.create_painter_with(
            [0, 0, image.width() as i32, image.height() as i32],
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }

    pub fn from_bytes(bytes: &[u8], world: &mut World) -> Result<Image, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;

        let interface = world.single_mut::<Interface>().unwrap();
        let painter = interface.create_painter_with(
            [0, 0, image.width() as i32, image.height() as i32],
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }
}
