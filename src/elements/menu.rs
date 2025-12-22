use wgpu::Color;
use winit::window::Window;

use crate::{
    elements::palette::PaletteDescriptor,
    lnwin::{Lnwindow, PointerAltEvent, PointerEvent},
    measures::{Position, Rectangle, Size},
    render::{
        Render,
        canvas::CanvasDescriptor,
        rounded::{RoundedRect, RoundedRectDescriptor},
        text::{Text, TextDescriptor},
    },
    tools::{
        pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave, PointerMenu},
        transform::TransformTool,
    },
    world::{Descriptor, Element, Handle, World},
};

const PAD: u32 = 10;
const PAD_TEXT: u32 = 8;

pub struct Menu {
    frame: RoundedRect,
    entries: Vec<MenuEntry>,
}

struct MenuEntry {
    frame: RoundedRect,
    text: Text,
    action: Box<dyn Fn(&World)>,
}

pub struct MenuDescriptor {
    pub position: Position,
    pub entry_width: u32,
    pub entry_height: u32,
    pub entries: Vec<MenuEntryDescriptor>,
}

pub struct MenuEntryDescriptor {
    pub label: String,
    pub action: Box<dyn Fn(&World)>,
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

impl Element for Menu {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider::fullscreen(80));

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| {
            let PointerEvent::Pressed(point) = event else {
                return;
            };

            let fetched = world.fetch(this).unwrap();
            let frame = fetched.frame.rect;

            if !frame.contains(point) {
                world.remove(this);
            }
        });

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.remove(this);

            let lnwindow = world.single::<Lnwindow>().unwrap();
            world.trigger(lnwindow, PointerAltEvent(position));
        });

        for (i, entry) in self.entries.iter().enumerate() {
            let collider = world.insert(PointerCollider {
                rect: entry.frame.rect.expand(PAD as i32 / 2),
                order: 110,
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
                fetched.entries[i].frame.visible = true;
                fetched.entries[i].frame.upload();
            });

            world.observer(collider, move |&PointerLeave, world, _| {
                let mut fetched = world.fetch_mut(this).unwrap();
                fetched.entries[i].frame.visible = false;
                fetched.entries[i].frame.upload();
            });
        }
    }
}

impl Descriptor for MenuDescriptor {
    type Target = Menu;

    fn build(self, world: &World) -> Self::Target {
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
            visible: true,
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

        Menu { frame, entries }
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
                    action: Box::new(move |world| {
                        world.insert(world.build(TextDescriptor {
                            rect: Rectangle {
                                origin: Position::default(),
                                extend: Size::splat(100),
                            },
                            text: "New Label",
                            ..Default::default()
                        }));
                    }),
                },
                MenuEntryDescriptor {
                    label: "New Palette".into(),
                    action: Box::new(move |world| {
                        world.insert(world.build(PaletteDescriptor::default()));
                    }),
                },
                MenuEntryDescriptor {
                    label: "LnDrawer".into(),
                    action: Box::new(move |world| {
                        let image = CanvasDescriptor::from_bytes(
                            Position::default(),
                            include_bytes!("../../res/iconv2.png"),
                        );
                        world.insert(world.build(image.unwrap()));
                    }),
                },
                MenuEntryDescriptor {
                    label: "Transform Tool".into(),
                    action: Box::new(move |world| {
                        world.insert(TransformTool::default());
                    }),
                },
                MenuEntryDescriptor {
                    label: "World Save".into(),
                    action: Box::new(move |world| {
                        crate::save::save_into_file(world);
                    }),
                },
                MenuEntryDescriptor {
                    label: "World Load".into(),
                    action: Box::new(move |world| {
                        crate::save::read_from_file(world);
                    }),
                },
                MenuEntryDescriptor {
                    label: "Switch Transparency".into(),
                    action: Box::new(move |world| {
                        let mut render = world.single_fetch_mut::<Render>().unwrap();
                        if render.clear_color == Color::TRANSPARENT {
                            render.clear_color = Color::BLACK;
                        } else if render.clear_color == Color::BLACK {
                            render.clear_color = Color::TRANSPARENT;
                        }
                    }),
                },
            ],
        }
    }
}
