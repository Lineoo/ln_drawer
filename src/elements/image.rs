use std::{error::Error, path::Path};

use crate::{
    elements::menu::{MenuDescriptor, MenuEntryDescriptor},
    interface::{Interface, Painter, PainterDescriptor},
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::{
        pointer::{PointerCollider, PointerMenu},
        transform::{Transform, TransformUpdate},
    },
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
            world.insert(world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            }));
        });

        let transform = world.insert(Transform {
            rect: self.painter.get_rect(),
            resizable: true,
        });

        world.dependency(transform, this);

        world.observer(transform, move |TransformUpdate, world, transform| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            let transform = world.fetch(transform).unwrap();

            this.painter.set_rect(transform.rect);
            collider.rect = transform.rect;
        });
    }
}

impl Image {
    pub fn new(descriptor: PainterDescriptor, interface: &mut Interface) -> Image {
        Image {
            painter: Painter::new(descriptor, interface),
        }
    }

    pub fn to_descriptor(&self) -> PainterDescriptor {
        self.painter.to_descriptor()
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
