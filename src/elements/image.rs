use std::{error::Error, path::Path};

use crate::{
    interface::{Interface, Painter},
    measures::{Delta, Position, Rectangle},
    world::{Element, Modifier, WorldCellEntry},
};

pub struct Image {
    painter: Painter,
}
impl Element for Image {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe::<Modifier<Position>>(move |modifier, entry| {
            let mut this = entry.fetch_mut::<Image>(entry.handle()).unwrap();
            let dest = modifier.invoke(this.painter.get_position());
            this.painter.set_position(dest);
        });
    }
}
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

    pub fn get_position(&self) -> Position {
        self.painter.get_position()
    }

    pub fn set_position(&mut self, position: Position) {
        self.painter.set_position(position);
    }

    pub fn get_rect(&self) -> Rectangle {
        self.painter.get_rect()
    }
}
