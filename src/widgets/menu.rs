use crate::{
    layout::Layout,
    measures::{Position, Rectangle, Size},
    theme::{Attach, Luni},
    tools::pointer::{PointerCollider, PointerHit, PointerHover, PointerMotion, PointerStatus},
    widgets::events::{WidgetButton, WidgetClick, WidgetDestroyed, WidgetHover, WidgetModified, WidgetSelect},
    world::{Descriptor, Element, Handle, World},
};

pub struct Menu {
    pub position: Position,
    pub entry_width: u32,
    pub entry_height: u32,
    pub pad: u32,
    pub hover: Option<i32>,
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

impl Default for MenuDescriptor {
    fn default() -> Self {
        MenuDescriptor {
            position: Position::default(),
            entry_width: 400,
            entry_height: 40,
            entry_pad: 5,
        }
    }
}

impl Descriptor for MenuDescriptor {
    type Target = Handle<Menu>;

    fn when_build(self, world: &World) -> Self::Target {
        let rect = Rectangle {
            origin: self.position,
            extend: Size::new(
                self.entry_pad + self.entry_width + self.entry_pad,
                self.entry_pad,
            ),
        };

        let collider = world.insert(PointerCollider {
            rect,
            order: 100,
            enabled: false,
        });

        let menu = world.insert(Menu {
            position: self.position,
            entry_width: self.entry_width,
            entry_height: self.entry_height,
            pad: self.entry_pad,
            hover: None,
            collider,
            entries: Vec::new(),
        });

        world.insert(Attach {
            widget: menu,
            theme: world.single::<Luni>().unwrap(),
        });

        world.queue(move |world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = true;
        });

        menu
    }
}

impl Element for Menu {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(self.collider, this);

        world.observer(
            self.collider,
            move |event: &PointerHover, world, _| match event.motion {
                PointerMotion::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerMotion::Moving => {
                    let mut this = world.fetch_mut(this).unwrap();

                    if event.position.within(this.menu_rect()) {
                        let idx = (event.position.y - this.position.y)
                            / (this.entry_height + this.pad) as i32;

                        let idx = idx.clamp(0, this.entries.len() as i32 - 1);

                        if this.hover.is_none_or(|x| x != idx) {
                            this.hover = Some(idx);
                            world.trigger(this.handle(), &WidgetSelect(Some(idx)));
                        }
                    } else if this.hover.is_some() {
                        this.hover = None;
                        world.trigger(this.handle(), &WidgetSelect(None));
                    }
                }
                PointerMotion::Leave => {
                    world.trigger(this, &WidgetSelect(None));
                    world.trigger(this, &WidgetHover::HoverLeave);

                    let mut this = world.fetch_mut(this).unwrap();
                    this.hover = None;
                }
            },
        );

        world.observer(
            self.collider,
            move |event: &PointerHit, world, _| match event.status {
                PointerStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerStatus::Moving => {
                    let mut this = world.fetch_mut(this).unwrap();

                    if event.position.within(this.menu_rect()) {
                        let idx = (event.position.y - this.position.y)
                            / (this.entry_height + this.pad) as i32;

                        let idx = idx.clamp(0, this.entries.len() as i32 - 1);

                        if this.hover.is_none_or(|x| x != idx) {
                            this.hover = Some(idx);
                            world.trigger(this.handle(), &WidgetSelect(Some(idx)));
                        }
                    } else if this.hover.is_some() {
                        this.hover = None;
                        world.trigger(this.handle(), &WidgetSelect(None));
                    }
                }
                PointerStatus::Release => {
                    world.trigger(this, &WidgetButton::ButtonRelease);

                    let mut this = world.fetch_mut(this).unwrap();

                    if event.position.within(this.menu_rect()) {
                        let idx = (event.position.y - this.position.y)
                            / (this.entry_height + this.pad) as i32;

                        if let Some(entry) = this.entries.get(idx as usize) {
                            world.trigger(*entry, &WidgetClick);
                        } else {
                            log::error!("menu hit nothing");
                        }

                        this.hover = None;
                    }
                }
            },
        );
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let mut collider = world.fetch_mut(self.collider).unwrap();
        collider.rect = self.menu_rect();

        for (i, entry) in self.entries.iter().enumerate() {
            world.trigger(*entry, &Layout::Rectangle(self.entry_rect(i as f32)));
        }

        world.queue(move |world| {
            world.trigger(this, &WidgetModified);
        });
    }

    // FIXME this is unavailable yet
    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        world.queue(move |world| {
            world.trigger(this, &WidgetDestroyed);
        });
    }
}

impl Descriptor for MenuEntryDescriptor {
    type Target = Handle<MenuEntry>;

    fn when_build(self, world: &World) -> Self::Target {
        let entry = world.insert(MenuEntry { menu: self.menu });

        world.queue(move |world| {
            let mut menu = world.fetch_mut(self.menu).unwrap();
            menu.entries.push(entry);
        });

        entry
    }
}

impl Element for MenuEntry {}

impl Menu {
    pub fn menu_rect(&self) -> Rectangle {
        Rectangle {
            origin: self.position,
            extend: Size::new(
                self.pad + self.entry_width + self.pad,
                self.pad + (self.entry_height + self.pad) * self.entries.len() as u32,
            ),
        }
    }

    pub fn entry_rect(&self, idx: f32) -> Rectangle {
        let offset = ((self.pad + self.entry_height) as f32 * idx).floor() as i32;
        Rectangle {
            origin: self.position + Position::splat(self.pad as i32) + Position::new(0, offset),
            extend: Size::new(self.entry_width, self.entry_height),
        }
    }
}
