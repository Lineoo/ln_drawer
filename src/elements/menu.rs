use crate::{
    elements::{Image, Palette},
    interface::{Interface, StandardSquare},
    lnwin::{Lnwindow, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    text::{Text, TextEdit, TextManager},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, ElementDescriptor, Handle, World},
};

const PAD: i32 = 10;
const PAD_H: i32 = PAD / 2;
const PAD_TEXT: i32 = 8;

const ENTRY_HEIGHT: i32 = 40;

pub struct Menu {
    frame: StandardSquare,
    select_frame: StandardSquare,
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
        let lnwindow = world.single::<Lnwindow>().unwrap();

        let obs = world.observer(lnwindow, move |event: &PointerEvent, world, _| {
            if let &PointerEvent::Moved(point) = event {
                let mut fetched = world.fetch_mut(this).unwrap();
                let frame = fetched.frame.get_rect();

                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                    let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    let rect = fetched.entries[index].frame.get_rect();
                    fetched.select_frame.set_rect(rect);
                    fetched.select_frame.set_visible(true);
                } else {
                    fetched.select_frame.set_visible(false);
                }
            } else if let &PointerEvent::Pressed(point) = event {
                let fetched = world.fetch(this).unwrap();
                let frame = fetched.frame.get_rect();

                if !frame.contains(point) {
                    world.remove(this);
                }
            }
        });

        world.dependency(obs, this);

        let collider = world.insert(PointerCollider {
            rect: self.frame.get_rect(),
            z_order: ZOrder::new(100),
        });

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| {
            let fetched = world.fetch(this).unwrap();
            let frame = fetched.frame.get_rect();

            if let PointerEvent::Pressed(point) = event {
                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                    let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    (fetched.entries[index].action)(world);
                    world.remove(this);
                }
            }
        });
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

        let select_frame = StandardSquare::new(
            rect,
            ZOrder::new(120),
            false,
            palette::Srgba::new(0.3, 0.3, 0.3, 1.0),
            interface,
        );

        let mut entries = Vec::with_capacity(descriptor.entries.len());
        for entry in descriptor.entries {
            let rect = Rectangle {
                origin: frame.get_rect().origin
                    + Delta::new(PAD, PAD + (ENTRY_HEIGHT + PAD) * entries.len() as i32),
                extend: Delta::new(descriptor.entry_width - PAD * 2, ENTRY_HEIGHT),
            };

            let frame = StandardSquare::new(
                rect,
                ZOrder::new(100),
                true,
                palette::Srgba::new(0.1, 0.1, 0.1, 0.0),
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

        Menu {
            frame,
            select_frame,
            entries,
        }
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
                                extend: Delta::new(300, 600),
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
