use std::{error::Error, path::Path};

use crate::{
    elements::menu::{MenuDescriptor, MenuEntryDescriptor},
    interface::{Interface, Painter},
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerMenu},
    world::{Element, Handle, World},
};

pub struct Image {
    painter: Painter,
}

impl Element for Image {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: self.painter.get_rect(),
            z_order: ZOrder::new(0),
        });

        world.dependency(collider, this);

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            });
        });
    }
}

impl Image {
    pub fn new(path: impl AsRef<Path>, interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let reader = image::ImageReader::open(path)?;
        let image = reader.decode()?;

        let painter = Painter::new_with(
            Rectangle {
                origin: Position::new(0, 0),
                extend: Delta::new(image.width() as i32, image.height() as i32),
            },
            Vec::from(image.as_bytes()),
            interface,
        );

        Ok(Image { painter })
    }

    pub fn from_bytes(bytes: &[u8], interface: &mut Interface) -> Result<Image, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;

        let painter = Painter::new_with(
            Rectangle {
                origin: Position::new(0, 0),
                extend: Delta::new(image.width() as i32, image.height() as i32),
            },
            Vec::from(image.as_bytes()),
            interface,
        );

        Ok(Image { painter })
    }
}
