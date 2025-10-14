use crate::{
    elements::{ButtonRaw, Image, Palette},
    interface::{Interface, Square},
    lnwin::{Lnwindow, PointerEvent},
    measures::{Delta, Position, Rectangle, ZOrder},
    text::{Text, TextEdit, TextManager},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, WorldCell, WorldCellEntry},
};

const PAD: i32 = 10;
const PAD_H: i32 = PAD / 2;
const ENTRY_NUM: usize = 5;
const ENTRY_WIDTH: i32 = 220;
const ENTRY_HEIGHT: i32 = 30;

struct MenuEntry {
    frame: Square,
    _text: Text,
    action: Box<dyn Fn(&WorldCell)>,
}
pub struct Menu {
    frame: Square,
    select_frame: Square,
    entries: Vec<MenuEntry>,
    collider: PointerCollider,
}
impl Element for Menu {
    fn when_inserted(&mut self, entry: WorldCellEntry) {
        let handle = entry.handle();
        let obs = entry
            .single_entry::<Lnwindow>()
            .unwrap()
            .observe::<PointerEvent>(move |event, world| {
                if let &PointerEvent::Moved(point) = event {
                    let mut this = world.fetch_mut::<Menu>(handle).unwrap();
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
                    let this = world.fetch::<Menu>(handle).unwrap();
                    let frame = this.frame.get_rect();
                    if !frame.contains(point) {
                        world.remove(handle);
                    }
                }
            });

        entry.entry(obs).unwrap().depend(handle);

        entry.observe::<PointerHit>(move |event, entry| {
            let this = entry.fetch::<Menu>(handle).unwrap();
            let frame = this.frame.get_rect();

            if let &PointerHit(PointerEvent::Pressed(point)) = event {
                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                    let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    (this.entries[index].action)(entry.world());
                    entry.remove(handle);
                }
            }
        });

        entry.getter::<PointerCollider>(|this| this.downcast_ref::<Menu>().unwrap().collider);
    }
}
impl Menu {
    pub fn new(
        position: Position,
        text_manager: &mut TextManager,
        interface: &mut Interface,
    ) -> Menu {
        let rect = Rectangle {
            origin: position,
            extend: Delta::new(
                PAD + (ENTRY_WIDTH + PAD),
                PAD + (ENTRY_HEIGHT + PAD) * ENTRY_NUM as i32,
            ),
        };
        let frame = interface.create_square(rect, [0.1, 0.1, 0.1, 1.0]);
        frame.set_z_order(ZOrder::new(90));

        let select_frame = interface.create_square(rect, [0.1, 0.1, 0.9, 0.2]);
        select_frame.set_z_order(ZOrder::new(120));
        select_frame.set_visible(false);

        let mut entries = Vec::new();
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(ZOrder::new(100));
            let mut _text = Text::new(rect, "Label".into(), text_manager, interface);
            _text.set_order(ZOrder::new(110));
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(Text::new(
                        rect,
                        "New Label".into(),
                        &mut world.single_fetch_mut().unwrap(),
                        &mut world.single_fetch_mut().unwrap(),
                    ));
                }),
            });
        }
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD + (PAD + ENTRY_HEIGHT)),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(ZOrder::new(100));
            let mut _text = Text::new(rect, "Palette".into(), text_manager, interface);
            _text.set_order(ZOrder::new(110));
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    let palette = Palette::new(rect.origin, &mut world.single_fetch_mut().unwrap());
                    world.insert(palette);
                }),
            });
        }
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD + (PAD + ENTRY_HEIGHT) * 2),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(ZOrder::new(100));
            let mut _text = Text::new(rect, "Button".into(), text_manager, interface);
            _text.set_order(ZOrder::new(110));
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(ButtonRaw::new(
                        Rectangle {
                            origin: rect.origin,
                            extend: Delta::new(100, 100),
                        },
                        |_| println!("Button hit!"),
                        &mut world.single_fetch_mut().unwrap(),
                    ));
                }),
            });
        }
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD + (PAD + ENTRY_HEIGHT) * 3),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(ZOrder::new(100));
            let mut _text = Text::new(rect, "LnDrawer Logo".into(), text_manager, interface);
            _text.set_order(ZOrder::new(110));
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    let image = Image::from_bytes(
                        include_bytes!("../../res/icon.png"),
                        &mut world.single_fetch_mut().unwrap(),
                    )
                    .unwrap();
                    world.insert(image);
                }),
            });
        }
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD + (PAD + ENTRY_HEIGHT) * 4),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(ZOrder::new(100));
            let mut _text = Text::new(rect, "Text Edit".into(), text_manager, interface);
            _text.set_order(ZOrder::new(110));
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(TextEdit::new(
                        Rectangle {
                            origin: Position::new(0, 0),
                            extend: Delta::splat(300),
                        },
                        "Enter text here".into(),
                        &mut world.single_fetch_mut().unwrap(),
                        &mut world.single_fetch_mut().unwrap(),
                    ));
                }),
            });
        }

        let collider = PointerCollider {
            rect: frame.get_rect(),
            z_order: ZOrder::new(100),
        };

        Menu {
            frame,
            select_frame,
            entries,
            collider,
        }
    }
}
