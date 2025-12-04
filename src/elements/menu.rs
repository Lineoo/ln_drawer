use crate::{
    elements::{Image, Palette},
    interface::{Interface, StandardSquare},
    lnwin::{Lnwindow, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    text::{Text, TextEdit, TextManager},
    tools::pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    world::{Element, ElementDescriptor, Handle, World},
};

const PAD: i32 = 10;
const PAD_TEXT: i32 = 8;

pub struct Menu {
    frame: StandardSquare,
    entries: Vec<MenuEntry>,
}

struct MenuEntry {
    frame: StandardSquare,
    text: Text,
    action: Box<dyn Fn(&World)>,
}

pub struct MenuDescriptor {
    pub position: Position,
    pub entry_width: i32,
    pub entry_height: i32,
    pub entries: Vec<MenuEntryDescriptor>,
}

pub struct MenuEntryDescriptor {
    pub label: String,
    pub action: Box<dyn Fn(&World)>,
}

impl Element for Menu {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider::fullscreen(ZOrder::new(80)));

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| {
            let PointerEvent::Pressed(point) = event else {
                return;
            };

            let fetched = world.fetch(this).unwrap();
            let frame = fetched.frame.get_rect();

            if !frame.contains(point) {
                world.remove(this);
            }
        });

        for (i, entry) in self.entries.iter().enumerate() {
            let collider = world.insert(PointerCollider {
                rect: entry.frame.get_rect(),
                z_order: ZOrder::new(110),
            });

            world.dependency(collider, this);

            world.observer(collider, move |&PointerHit(event), world, _| {
                let PointerEvent::Pressed(_) = event else {
                    return;
                };

                let fetched = world.fetch(this).unwrap();
                (fetched.entries[i].action)(world);
                world.remove(this);
            });

            world.observer(collider, move |&PointerEnter, world, _| {
                let mut fetched = world.fetch_mut(this).unwrap();
                fetched.entries[i].frame.set_visible(true);
            });

            world.observer(collider, move |&PointerLeave, world, _| {
                let mut fetched = world.fetch_mut(this).unwrap();
                fetched.entries[i].frame.set_visible(false);
            });
        }
    }
}

impl ElementDescriptor for MenuDescriptor {
    type Target = Menu;

    fn build(self, world: &World) -> Self::Target {
        Menu::new(
            self,
            &mut world.single_fetch_mut().unwrap(),
            &mut world.single_fetch_mut().unwrap(),
        )
    }
}

impl Menu {
    pub fn new(
        descriptor: MenuDescriptor,
        text_manager: &mut TextManager,
        interface: &mut Interface,
    ) -> Menu {
        let rect = Rectangle {
            origin: descriptor.position,
            extend: Delta::new(
                PAD + (descriptor.entry_width + PAD),
                PAD + (descriptor.entry_height + PAD) * descriptor.entries.len() as i32,
            ),
        };

        let frame = StandardSquare::new(
            rect,
            ZOrder::new(90),
            true,
            palette::Srgba::new(0.1, 0.1, 0.1, 0.9),
            interface,
        );

        let mut entries = Vec::with_capacity(descriptor.entries.len());
        for entry in descriptor.entries {
            let prev = (descriptor.entry_height + PAD) * entries.len() as i32;
            let rect = Rectangle {
                origin: frame.get_rect().origin + Delta::new(PAD, PAD + prev),
                extend: Delta::new(descriptor.entry_width, descriptor.entry_height),
            };

            let frame = StandardSquare::new(
                rect,
                ZOrder::new(120),
                false,
                palette::Srgba::new(0.3, 0.3, 0.3, 1.0),
                interface,
            );

            let mut text = Text::new(
                Rectangle {
                    origin: rect.origin + Delta::splat(PAD_TEXT),
                    extend: rect.extend - Delta::splat(PAD_TEXT * 2),
                },
                entry.label,
                text_manager,
                interface,
            );
            text.set_order(ZOrder::new(140));

            entries.push(MenuEntry {
                frame,
                text,
                action: entry.action,
            });
        }

        Menu { frame, entries }
    }

    pub fn test_descriptor(position: Position) -> MenuDescriptor {
        MenuDescriptor {
            position,
            entry_width: 400,
            entry_height: 40,
            entries: vec![
                MenuEntryDescriptor {
                    label: "New Label".into(),
                    action: Box::new(move |world| {
                        world.insert(Text::new(
                            Rectangle {
                                origin: Position::default(),
                                extend: Delta::new(100, 100),
                            },
                            "New Label".into(),
                            &mut world.single_fetch_mut().unwrap(),
                            &mut world.single_fetch_mut().unwrap(),
                        ));
                    }),
                },
                MenuEntryDescriptor {
                    label: "New Palette".into(),
                    action: Box::new(move |world| {
                        let palette = Palette::new(
                            Position::default(),
                            &mut world.single_fetch_mut().unwrap(),
                        );
                        world.insert(palette);
                    }),
                },
                MenuEntryDescriptor {
                    label: "LnDrawer".into(),
                    action: Box::new(move |world| {
                        let image = Image::from_bytes(
                            include_bytes!("../../res/icon.png"),
                            &mut world.single_fetch_mut().unwrap(),
                        )
                        .unwrap();
                        world.insert(image);
                    }),
                },
                MenuEntryDescriptor {
                    label: "New TextEdit".into(),
                    action: Box::new(move |world| {
                        world.insert(TextEdit::new(
                            Rectangle {
                                origin: Position::default(),
                                extend: Delta::new(600, 200),
                            },
                            "Enter text here".into(),
                            &mut world.single_fetch_mut().unwrap(),
                            &mut world.single_fetch_mut().unwrap(),
                        ));
                    }),
                },
            ],
        }
    }
}
