use crate::{
    elements::{
        ButtonRaw, Element, Image, OrderElement, Palette, PositionElementExt, PositionedElement,
        Text,
        text::{TextEdit, TextManager},
    },
    interface::{Interface, Square},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle},
    tools::pointer::{PointerHit, PointerHitExt, PointerHittable},
    world::{ElementHandle, ElementInserted, WorldCell},
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
}
impl Element for Menu {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let obs = world.observe::<PointerEvent>(move |event, world| {
            if let &PointerEvent::Moved(point) = event {
                let mut this = world.fetch_mut::<Menu>(handle).unwrap();
                let frame = this.frame.get_rect();

                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.w > PAD_H && delta1.h > PAD_H && delta2.w > PAD_H && delta2.h > PAD_H {
                    let index = ((delta1.h - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
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

        world.entry(obs).unwrap().depend(handle);

        (world.entry(handle).unwrap()).observe::<PointerHit>(move |event, world| {
            let this = world.fetch::<Menu>(handle).unwrap();
            let frame = this.frame.get_rect();

            if let &PointerHit(PointerEvent::Pressed(point)) = event {
                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.w > PAD_H && delta1.h > PAD_H && delta2.w > PAD_H && delta2.h > PAD_H {
                    let index = ((delta1.h - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    (this.entries[index].action)(world);
                    world.remove(handle);
                }
            }
        });

        let obs = world.observe::<ElementInserted>(move |event, world| {
            if world.contains_type::<Menu>(event.0) && event.0 != handle {
                world.remove(handle);
            }
        });

        world.entry(obs).unwrap().depend(handle);

        self.register_position(handle, world);
        self.register_hittable(handle, world);
    }
}
impl PositionedElement for Menu {
    fn get_position(&self) -> Position {
        self.frame.get_position()
    }

    fn set_position(&mut self, position: Position) {
        self.frame.set_position(position);
    }
}
impl PointerHittable for Menu {
    fn get_hitting_rect(&self) -> Rectangle {
        self.frame.get_rect()
    }

    fn get_hitting_order(&self) -> isize {
        100
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
        frame.set_z_order(90);

        let select_frame = interface.create_square(rect, [0.1, 0.1, 0.9, 0.2]);
        select_frame.set_z_order(120);
        select_frame.set_visible(false);

        let mut entries = Vec::new();
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect, [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(100);
            let mut _text = Text::new(rect, "Label".into(), text_manager, interface);
            _text.set_order(110);
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(Text::new(
                        rect,
                        "New Label".into(),
                        &mut world.single_mut().unwrap(),
                        &mut world.single_mut().unwrap(),
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
            frame.set_z_order(100);
            let mut _text = Text::new(rect, "Palette".into(), text_manager, interface);
            _text.set_order(110);
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    let palette = Palette::new(rect.origin, &mut world.single_mut().unwrap());
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
            frame.set_z_order(100);
            let mut _text = Text::new(rect, "Button".into(), text_manager, interface);
            _text.set_order(110);
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
            frame.set_z_order(100);
            let mut _text = Text::new(rect, "LnDrawer Logo".into(), text_manager, interface);
            _text.set_order(110);
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    let image = Image::from_bytes(
                        include_bytes!("../../res/icon.png"),
                        &mut world.single_mut().unwrap(),
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
            frame.set_z_order(100);
            let mut _text = Text::new(rect, "Text Edit".into(), text_manager, interface);
            _text.set_order(110);
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(TextEdit::new(
                        rect,
                        "Enter text here".into(),
                        &mut world.single_mut().unwrap(),
                        &mut world.single_mut().unwrap(),
                    ));
                }),
            });
        }

        Menu {
            frame,
            select_frame,
            entries,
        }
    }
}
