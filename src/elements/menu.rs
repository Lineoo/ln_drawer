use wgpu::Color;
use winit::window::WindowLevel;

use crate::{
    elements::{palette::PaletteDescriptor, panel::ExPanelDescriptor},
    layout::resizable::ResizableDescriptor,
    lnwin::Lnwindow,
    measures::{Position, Rectangle, Size},
    render::{
        Render, RenderPortal,
        canvas::CanvasDescriptor,
        rounded::{RoundedRect, RoundedRectDescriptor},
        text::{Text, TextDescriptor},
    },
    theme::{Attach, Luni},
    tools::pointer::{
        PointerCollider, PointerHit, PointerHover, PointerMenu, PointerStatus, PointerTool,
    },
    widgets::{
        button::ButtonDescriptor,
        check_button::CheckButtonDescriptor,
        events::{Click, Switch},
        panel::PanelDescriptor,
    },
    world::{Descriptor, Element, Handle, World},
};

const PAD: u32 = 10;
const PAD_TEXT: u32 = 8;

pub struct Menu {
    frame: Handle<RoundedRect>,
    entries: Vec<MenuEntry>,
}

struct MenuEntry {
    text: Handle<Text>,
    frame: Handle<RoundedRect>,
    action: Box<dyn Fn(&World, Position)>,
}

pub struct MenuDescriptor {
    pub position: Position,
    pub entry_width: u32,
    pub entry_height: u32,
    pub entries: Vec<MenuEntryDescriptor>,
}

pub struct MenuEntryDescriptor {
    pub label: String,
    pub action: Box<dyn Fn(&World, Position)>,
}

impl Default for MenuDescriptor {
    fn default() -> Self {
        Self {
            position: Position::default(),
            entry_width: 400,
            entry_height: 40,
            entries: Vec::new(),
        }
    }
}

impl Descriptor for MenuDescriptor {
    type Target = Handle<Menu>;

    fn when_build(self, world: &World) -> Self::Target {
        let rect = Rectangle {
            origin: self.position,
            extend: Size::new(
                PAD + (self.entry_width + PAD),
                PAD + (self.entry_height + PAD) * self.entries.len() as u32,
            ),
        };

        let frame = world.build(RoundedRectDescriptor {
            rect,
            color: palette::Srgba::new(0.1, 0.1, 0.1, 0.9),
            order: 90,
            ..Default::default()
        });

        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let prev = (self.entry_height + PAD) * entries.len() as u32;
            let rect = Rectangle {
                origin: rect.origin.wrapping_add(Size::new(PAD, PAD + prev)),
                extend: Size::new(self.entry_width, self.entry_height),
            };

            let frame = world.build(RoundedRectDescriptor {
                rect,
                color: palette::Srgba::new(0.3, 0.3, 0.3, 1.0),
                order: 120,
                visible: false,
                ..Default::default()
            });

            let text = world.build(TextDescriptor {
                text: &entry.label,
                rect: rect.expand(-PAD_TEXT.cast_signed()),
                order: 140,
                ..Default::default()
            });

            entries.push(MenuEntry {
                frame,
                text,
                action: entry.action,
            });
        }

        world.insert(Menu { frame, entries })
    }
}

impl Element for Menu {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider::fullscreen(80));

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world, _| {
            if let PointerStatus::Press = event.status {
                return;
            };

            let fetched = world.fetch(this).unwrap();
            let frame = world.fetch(fetched.frame).unwrap();
            let frame = frame.rect;

            if !event.position.within(frame) {
                world.remove(this);
            }
        });

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.remove(this);

            world.queue(move |world| {
                let pointer = world.single::<PointerTool>().unwrap();
                world.trigger(pointer, PointerMenu(position));
            });
        });

        world.dependency(self.frame, this);

        for (i, entry) in self.entries.iter().enumerate() {
            let frame = world.fetch(entry.frame).unwrap();
            let collider = world.insert(PointerCollider {
                rect: frame.rect.expand(PAD as i32 / 2),
                order: 110,
            });

            world.dependency(collider, this);

            world.observer(collider, move |event: &PointerHit, world, _| {
                if let PointerStatus::Press = event.status {
                    return;
                };

                let fetched = world.fetch(this).unwrap();
                let frame = world.fetch(fetched.frame).unwrap();
                (fetched.entries[i].action)(world, frame.rect.origin);
                world.remove(this);
            });

            world.observer(collider, move |event: &PointerHover, world, _| {
                let fetched = world.fetch(this).unwrap();
                let mut frame = world.fetch_mut(fetched.entries[i].frame).unwrap();
                frame.visible = match event {
                    PointerHover::Enter => true,
                    PointerHover::Leave => false,
                };
            });

            world.dependency(entry.frame, this);
            world.dependency(entry.text, this);
        }
    }
}

impl Menu {
    pub fn test_descriptor(position: Position) -> MenuDescriptor {
        MenuDescriptor {
            position,
            entry_width: 400,
            entry_height: 40,
            entries: vec![
                MenuEntryDescriptor {
                    label: "New Label".into(),
                    action: Box::new(move |world, position| {
                        world.build(TextDescriptor {
                            rect: Rectangle {
                                origin: position,
                                extend: Size::splat(100),
                            },
                            text: "New Label",
                            ..Default::default()
                        });
                    }),
                },
                MenuEntryDescriptor {
                    label: "New Palette".into(),
                    action: Box::new(move |world, position| {
                        world.build(PaletteDescriptor {
                            position,
                            ..Default::default()
                        });
                    }),
                },
                MenuEntryDescriptor {
                    label: "LnDrawer".into(),
                    action: Box::new(move |world, position| {
                        let image = CanvasDescriptor::from_bytes(
                            position,
                            include_bytes!("../../res/iconv2.png"),
                        );
                        world.build(image.unwrap());
                    }),
                },
                MenuEntryDescriptor {
                    label: "World Save".into(),
                    action: Box::new(move |world, _| {
                        crate::save::save_into_file(world);
                    }),
                },
                MenuEntryDescriptor {
                    label: "World Load".into(),
                    action: Box::new(move |world, _| {
                        crate::save::read_from_file(world);
                    }),
                },
                MenuEntryDescriptor {
                    label: "Switch Transparency".into(),
                    action: Box::new(move |world, _| {
                        let mut rportal = world.single_fetch_mut::<RenderPortal>().unwrap();
                        if rportal.clear_color == Color::TRANSPARENT {
                            rportal.clear_color = Color::BLACK;
                        } else if rportal.clear_color == Color::BLACK {
                            rportal.clear_color = Color::TRANSPARENT;
                        }
                    }),
                },
                MenuEntryDescriptor {
                    label: "Switch Title Bar".into(),
                    action: Box::new(move |world, _| {
                        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                        let decorated = lnwindow.window.is_decorated();
                        lnwindow.window.set_decorations(!decorated);
                    }),
                },
                MenuEntryDescriptor {
                    label: "Always On Top".into(),
                    action: Box::new(move |world, _| {
                        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                        lnwindow.window.set_window_level(WindowLevel::AlwaysOnTop);
                    }),
                },
                MenuEntryDescriptor {
                    label: "Cancel Always On Top".into(),
                    action: Box::new(move |world, _| {
                        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                        lnwindow.window.set_window_level(WindowLevel::Normal);
                    }),
                },
                MenuEntryDescriptor {
                    label: "A Button With Luni".into(),
                    action: Box::new(move |world, position| {
                        let luni = world.single::<Luni>().unwrap();
                        let button = world.build(ButtonDescriptor {
                            rect: Rectangle {
                                origin: position,
                                extend: Size::splat(100),
                            },
                            order: 20,
                        });

                        world.queue(move |world| {
                            world.trigger(luni, Attach(button));
                        });

                        world.observer(button, |Click, world, _| {
                            let mut rportal = world.single_fetch_mut::<RenderPortal>().unwrap();
                            if rportal.clear_color == Color::TRANSPARENT {
                                rportal.clear_color = Color::BLACK;
                            } else if rportal.clear_color == Color::BLACK {
                                rportal.clear_color = Color::TRANSPARENT;
                            }
                        });
                    }),
                },
                MenuEntryDescriptor {
                    label: "A Check Button With Luni".into(),
                    action: Box::new(move |world, position| {
                        let luni = world.single::<Luni>().unwrap();
                        let button = world.build(CheckButtonDescriptor {
                            rect: Rectangle {
                                origin: position,
                                extend: Size::splat(100),
                            },
                            checked: false,
                            order: 20,
                        });

                        world.queue(move |world| {
                            world.trigger(luni, Attach(button));
                        });

                        world.observer(button, |Switch, world, button| {
                            let mut button = world.fetch_mut(button).unwrap();
                            button.checked = !button.checked;
                        });
                    }),
                },
                MenuEntryDescriptor {
                    label: "A Panel With Luni and Resizable".into(),
                    action: Box::new(move |world, position| {
                        let panel = world.build(PanelDescriptor {
                            rect: Rectangle {
                                origin: position,
                                extend: Size::splat(100),
                            },
                            order: 0,
                        });

                        world.build(ResizableDescriptor {
                            rect: Rectangle {
                                origin: position,
                                extend: Size::splat(100),
                            },
                            order: 5,
                            target: panel.untyped(),
                        });

                        world.queue(move |world| {
                            let luni = world.single::<Luni>().unwrap();
                            world.trigger(luni, Attach(panel));
                        });
                    }),
                },
                MenuEntryDescriptor {
                    label: "[ex] Panel".into(),
                    action: Box::new(move |world, position| {
                        world.build(ExPanelDescriptor {
                            rounded: RoundedRectDescriptor {
                                rect: Rectangle {
                                    origin: position,
                                    extend: Size::splat(100),
                                },
                                color: palette::Srgba::new(0.82, 0.87, 1.00, 0.60),
                                visible: true,
                                ..Default::default()
                            },
                        });
                    }),
                },
            ],
        }
    }
}
