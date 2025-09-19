use crate::{
    elements::{
        ButtonRaw, Element, Image, Label, Palette, PositionedElement,
        intersect::{IntersectFail, IntersectHit, Intersection, PointerHover, PointerLeave},
    },
    interface::{Interface, Square, Text},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle},
    world::{ElementHandle, ElementInserted, WorldCell},
};

const PAD: i32 = 10;
const PAD_H: i32 = PAD / 2;
const ENTRY_NUM: usize = 4;
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
        (world.entry(handle).unwrap()).observe::<PointerHover>(move |event, world| {
            let mut this = world.fetch_mut::<Menu>(handle).unwrap();
            let frame = Rectangle::from_array(this.frame.get_rect());

            let &PointerHover(point) = event;
            let delta1 = point - frame.origin;
            let delta2 = frame.right_up() - point;
            if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                let rect = this.entries[index].frame.get_rect();
                this.select_frame.set_rect(rect);
                this.select_frame.set_visible(true);
            } else {
                this.select_frame.set_visible(false);
            }
        });

        (world.entry(handle).unwrap()).observe::<PointerLeave>(move |_event, world| {
            let this = world.fetch_mut::<Menu>(handle).unwrap();
            this.select_frame.set_visible(false);
        });

        (world.entry(handle).unwrap()).observe::<IntersectHit>(move |event, world| {
            let this = world.fetch::<Menu>(handle).unwrap();
            let frame = Rectangle::from_array(this.frame.get_rect());

            if let &IntersectHit(PointerEvent::Pressed(point)) = event {
                let delta1 = point - frame.origin;
                let delta2 = frame.right_up() - point;
                if delta1.x > PAD_H && delta1.y > PAD_H && delta2.x > PAD_H && delta2.y > PAD_H {
                    let index = ((delta1.y - PAD_H) / (ENTRY_HEIGHT + PAD)) as usize;
                    (this.entries[index].action)(world);
                    world.remove(handle);
                }
            }
        });

        let intersect = world.insert(Intersection {
            host: handle,
            rect: Rectangle::from_array(self.frame.get_rect()),
            z_order: 120,
        });
        world.entry(intersect).unwrap().depend(handle);

        (world.entry(handle).unwrap()).observe::<IntersectFail>(move |_event, world| {
            world.remove(handle);
        });

        (world.entry(handle).unwrap()).observe::<ElementInserted>(move |event, world| {
            if world.contains_type::<Menu>(event.0) && event.0 != handle {
                world.remove(handle);
            }
        });
    }
}
impl PositionedElement for Menu {
    fn get_position(&self) -> Position {
        Position::from_array(self.frame.get_position())
    }

    fn set_position(&mut self, position: Position) {
        self.frame.set_position(position.into_array());
    }
}
impl Menu {
    pub fn new(position: Position, interface: &mut Interface) -> Menu {
        let rect = Rectangle {
            origin: position,
            extend: Delta::new(
                PAD + (ENTRY_WIDTH + PAD),
                PAD + (ENTRY_HEIGHT + PAD) * ENTRY_NUM as i32,
            ),
        };
        let frame = interface.create_square(rect.into_array(), [0.1, 0.1, 0.1, 1.0]);
        frame.set_z_order(90);

        let select_frame = interface.create_square(rect.into_array(), [0.1, 0.1, 0.9, 0.2]);
        select_frame.set_z_order(120);
        select_frame.set_visible(false);

        let mut entries = Vec::new();
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect.into_array(), [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(100);
            let mut _text = interface.create_text(rect.into_array(), "Label");
            _text.set_z_order(110);
            entries.push(MenuEntry {
                frame,
                _text,
                action: Box::new(move |world| {
                    world.insert(Label::new(rect, "New Label".into(), world));
                }),
            });
        }
        {
            let rect = Rectangle {
                origin: position + Delta::new(PAD, PAD + (PAD + ENTRY_HEIGHT)),
                extend: Delta::new(ENTRY_WIDTH, ENTRY_HEIGHT),
            };
            let frame = interface.create_square(rect.into_array(), [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(100);
            let mut _text = interface.create_text(rect.into_array(), "Palette");
            _text.set_z_order(110);
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
            let frame = interface.create_square(rect.into_array(), [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(100);
            let mut _text = interface.create_text(rect.into_array(), "Button");
            _text.set_z_order(110);
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
            let frame = interface.create_square(rect.into_array(), [0.2, 0.2, 0.2, 1.0]);
            frame.set_z_order(100);
            let mut _text = interface.create_text(rect.into_array(), "LnDrawer Logo");
            _text.set_z_order(110);
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

        Menu {
            frame,
            select_frame,
            entries,
        }
    }
}
