use std::{error::Error, path::Path};

use crate::{
    elements::{OrderElement, OrderElementExt, intersect::Collider},
    interface::{Interface, Painter},
    measures::{Delta, Position, Rectangle},
    tools::pointer::PointerHittable,
    world::{Element, Modifier, WorldCellEntry},
};

pub struct Image {
    painter: Painter,
}
impl Element for Image {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        let intersect = entry.insert(Collider {
            rect: self.painter.get_rect(),
            z_order: 0,
        });
        // Collider service instead
        entry.entry(intersect).unwrap().depend(entry.handle());
        entry.observe::<Modifier<Position>>(move |modifier, entry| {
            let mut this = entry.fetch_mut::<Image>(entry.handle()).unwrap();
            let dest = modifier.invoke(this.painter.get_position());
            this.painter.set_position(dest);
            let mut intersect = entry.fetch_mut::<Collider>(intersect).unwrap();
            intersect.rect.origin = dest;
        });

        self.register_order(entry.handle(), entry.world());
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
impl PointerHittable for Image {
    fn get_hitting_rect(&self) -> Rectangle {
        self.painter.get_rect()
    }

    fn get_hitting_order(&self) -> isize {
        self.painter.get_z_order()
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
}
