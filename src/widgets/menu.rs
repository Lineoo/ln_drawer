use crate::{
    measures::{Position, Rectangle, Size}, render::{rounded::RoundedRect, text::Text}, tools::pointer::{PointerCollider, PointerMotion}, widgets::events::Interact, world::{Descriptor, Element, Handle, World}
};

pub struct Menu {
    pub position: Position,
    pub entry_width: u32,
    pub entry_height: u32,
    pub entry_pad: u32,
    collider: Handle<PointerCollider>,
    entries: Vec<Handle<MenuEntry>>,
}

pub struct MenuEntry {
    menu: Handle<Menu>,
}

pub struct MenuDescriptor {
    pub position: Position,
    pub entry_width: u32,
    pub entry_height: u32,
    pub entry_pad: u32,
}

pub struct MenuEntryDescriptor {
    pub menu: Handle<Menu>,
}

impl Descriptor for MenuDescriptor {
    type Target = Handle<Menu>;

    fn when_build(self, world: &World) -> Self::Target {
        let collider = world.insert(PointerCollider {
            rect: Rectangle {
                origin: self.position,
                extend: Size::new(self.entry_width, self.entry_pad),
            },
            order: 0,
            enabled: true,
        });

        world.insert(Menu {
            position: self.position,
            entry_width: self.entry_width,
            entry_height: self.entry_height,
            entry_pad: self.entry_pad,
            collider,
            entries: Vec::new(),
        })
    }
}

impl Element for Menu {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            self.collider,
            move |event: &PointerMotion, world, _| match event {
                PointerMotion::Enter => {
                    world.trigger(this, &Interact::HoverEnter);
                }
                PointerMotion::Moving => {

                }
                PointerMotion::Leave => {
                    world.trigger(this, &Interact::HoverLeave);
                }
            },
        );
    }
}

impl Descriptor for MenuEntryDescriptor {
    type Target = Handle<MenuEntry>;

    fn when_build(self, world: &World) -> Self::Target {
        let mut menu = world.fetch_mut(self.menu).unwrap();

        let entry = world.insert(MenuEntry { menu: self.menu });
        menu.entries.push(entry);

        entry
    }
}

impl Element for MenuEntry {}
