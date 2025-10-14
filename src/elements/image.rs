use std::{error::Error, path::Path};

use crate::{
    interface::{Interface, Painter},
    measures::{Delta, Position, Rectangle},
    world::{Element, InsertElement},
};

pub struct Image {
    painter: Painter,
}
impl Element for Image {}
impl InsertElement for Image {}
impl Image {
    pub fn new(path: impl AsRef<Path>, interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let reader = image::ImageReader::open(path)?;
        let image = reader.decode()?;

        let painter = interface.create_painter_with(
            Rectangle {
                origin: Position::new(0, 0),
                extend: Delta::new(image.width() as i32, image.height() as i32),
            },
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }

    pub fn from_bytes(bytes: &[u8], interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;

        let painter = interface.create_painter_with(
            Rectangle {
                origin: Position::new(0, 0),
                extend: Delta::new(image.width() as i32, image.height() as i32),
            },
            Vec::from(image.as_bytes()),
        );

        Ok(Image { painter })
    }

    pub fn get_rect(&self) -> Rectangle {
        self.painter.get_rect()
    }
}
