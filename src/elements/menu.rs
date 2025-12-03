use crate::{
    elements::{ButtonRaw, Image, Palette},
    interface::{Interface, StandardSquare},
    lnwin::{Lnwindow, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    text::{Text, TextEdit, TextManager},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, ElementDescriptor, WorldCell, WorldCellEntry},
};

const PAD: i32 = 10;
const PAD_H: i32 = PAD / 2;
const PAD_TEXT: i32 = 8;

const ENTRY_WIDTH: i32 = 300;
const ENTRY_HEIGHT: i32 = 40;

pub struct Menu {
    frame: StandardSquare,
    select_frame: StandardSquare,
    entries: Vec<MenuEntry>,
    collider: PointerCollider,
}

struct MenuEntry {
    frame: StandardSquare,
    text: Text,
    action: Box<dyn Fn(&WorldCell)>,
}

pub struct MenuDescriptor {
    pub position: Position,
    pub entry_width: i32,
    pub entry_height: i32,
    pub entries: Vec<MenuEntryDescriptor>,
}

pub struct MenuEntryDescriptor {
    pub label: String,
    pub action: Box<dyn Fn(&WorldCell)>,
}

impl Element for Menu {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        let handle = entry.handle();
        let obs = entry
            .single_entry::<Lnwindow>()
            .unwrap()
            .observe::<PointerEvent>(move |event, entry| {
                if let &PointerEvent::Moved(point) = event {
                    let mut this = entry.world().fetch_mut(handle).unwrap();
                    let frame = this.frame.get_rect();

                    let delta1 = point - frame.origin;
                    let delta2 = frame.right_up() - point;
                    if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H
                    {
                        let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                        let rect = this.entries[index].frame.get_rect();
                        this.select_frame.set_rect(rect);
                        this.select_frame.set_visible(true);
                    } else {
                        this.select_frame.set_visible(false);
                    }
                } else if let &PointerEvent::Pressed(point) = event {
                    let this = entry.world().fetch(handle).unwrap();
                    let frame = this.frame.get_rect();
                    if !frame.contains(point) {
                        entry.remove(handle.untyped());
                    }
                }
            });

        entry.entry(obs).unwrap().depend(handle.untyped());

        entry.observe::<PointerHit>(move |event, entry| {
            let this = entry.world().fetch(handle).unwrap();
            let frame = this.frame.get_rect();

            if let &PointerHit(PointerEvent::Pressed(point)) = event {
                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                    let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    (this.entries[index].action)(entry.world());
                    entry.remove(handle.untyped());
                }
            }
        });

        entry.getter::<PointerCollider>(|this| this.collider);
    }
}

impl ElementDescriptor for MenuDescriptor {
    type Target = Menu;

    fn build(self, world: &WorldCell) -> Self::Target {
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

        let collider = PointerCollider {
            rect: frame.get_rect(),
            z_order: ZOrder::new(100),
        };

        let mut entries = Vec::with_capacity(descriptor.entries.len());
        for entry in descriptor.entries {
            let rect = Rectangle {
                origin: frame.get_rect().origin
                    + Delta::new(PAD, PAD + (ENTRY_HEIGHT + PAD) * entries.len() as i32),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
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
            collider,
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
                    label: "New ButtonRaw".into(),
                    action: Box::new(move |world| {
                        world.insert(ButtonRaw::shell(
                            Rectangle {
                                origin: Position::default(),
                                extend: Delta::new(100, 100),
                            },
                            ZOrder::new(0),
                            &mut world.single_fetch_mut().unwrap(),
                        ));
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
