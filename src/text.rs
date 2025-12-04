use std::sync::Arc;

use cosmic_text::*;
use parking_lot::Mutex;
use winit::keyboard::{Key, NamedKey};

use crate::{
    interface::{Interface, Painter, Redraw},
    lnwin::{LnwinModifiers, PointerEvent},
    measures::{Position, Rectangle, ZOrder},
    tools::{
        focus::{Focus, FocusInput, RequestFocus},
        pointer::{PointerCollider, PointerHit},
    },
    world::{Element, Handle, World},
};

pub struct TextManager {
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,
}
impl Default for TextManager {
    fn default() -> Self {
        let font_system = Arc::new(Mutex::new(FontSystem::new()));
        let swash_cache = Arc::new(Mutex::new(SwashCache::new()));
        TextManager {
            font_system,
            swash_cache,
        }
    }
}
impl Element for TextManager {}

pub struct Text {
    inner: Painter,
}
impl Text {
    pub fn new(
        rect: Rectangle,
        text: String,
        manager: &mut TextManager,
        interface: &mut Interface,
    ) -> Text {
        let mut font_system = manager.font_system.lock();
        let mut swash_cache = manager.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut font_system);

        let attrs = Attrs::new();
        buffer_borrow.set_size(Some(rect.width() as f32), Some(rect.height() as f32));
        buffer_borrow.set_text(&text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);

        let mut data = vec![0; (rect.width() * rect.height() * 4) as usize];

        buffer_borrow.draw(
            &mut swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * rect.width() as i32) * 4) as usize;
                let rgba = color.as_rgba();
                if start >= data.len() {
                    return;
                }
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        Text {
            inner: Painter::new_with(rect, data, interface),
        }
    }

    pub fn get_order(&self) -> ZOrder {
        self.inner.get_z_order()
    }

    pub fn set_order(&mut self, order: ZOrder) {
        self.inner.set_z_order(order);
    }
}
impl Element for Text {}

pub struct TextEdit {
    inner: Painter,
    editor: Editor<'static>,
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,

    redraw: bool,
}

impl Element for TextEdit {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: self.inner.get_rect(),
            z_order: self.inner.get_z_order(),
        });

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Pressed(position) => {
                let fetched = &mut *world.fetch_mut(this).unwrap();

                let point = position - fetched.inner.get_rect().left_up();
                let point = Position::new(point.x, -point.y);

                let mut font_system = fetched.font_system.lock();
                fetched.editor.action(
                    &mut font_system,
                    Action::Click {
                        x: point.x,
                        y: point.y,
                    },
                );

                drop(font_system);

                let focus = world.single::<Focus>().unwrap();
                world.trigger(focus, RequestFocus(Some(this.untyped())));

                fetched.redraw = true;
            }
            PointerEvent::Moved(position) => {
                let fetched = &mut *world.fetch_mut(this).unwrap();

                let point = position - fetched.inner.get_rect().left_up();
                let point = Position::new(point.x, -point.y);

                let mut font_system = fetched.font_system.lock();
                fetched.editor.action(
                    &mut font_system,
                    Action::Drag {
                        x: point.x,
                        y: point.y,
                    },
                );

                drop(font_system);

                fetched.redraw = true;
            }
            _ => {}
        });

        world.observer(this, |FocusInput(event), world, this| {
            if !event.state.is_pressed() {
                return;
            }

            let fetched = &mut *world.fetch_mut(this).unwrap();

            let mut font_system = fetched.font_system.lock();
            let mut editor = fetched.editor.borrow_with(&mut font_system);

            let modifiers = world.single_fetch::<LnwinModifiers>().unwrap();
            let ctrl_down = modifiers.0.state().control_key();
            let shift_down = modifiers.0.state().shift_key();

            if shift_down && let Selection::None = editor.selection() {
                let cursor = editor.cursor();
                editor.set_selection(Selection::Normal(cursor));
            }

            match &event.logical_key {
                Key::Named(NamedKey::ArrowLeft) if ctrl_down => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::LeftWord))
                }
                Key::Named(NamedKey::ArrowRight) if ctrl_down => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::RightWord))
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Left));
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Right));
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Up));
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Down));
                }
                Key::Named(NamedKey::Home) => editor.action(Action::Motion(Motion::Home)),
                Key::Named(NamedKey::End) => editor.action(Action::Motion(Motion::End)),
                Key::Named(NamedKey::PageUp) => editor.action(Action::Motion(Motion::PageUp)),
                Key::Named(NamedKey::PageDown) => editor.action(Action::Motion(Motion::PageDown)),
                Key::Named(NamedKey::Escape) => editor.action(Action::Escape),
                Key::Named(NamedKey::Enter) => {
                    editor.delete_selection();
                    editor.action(Action::Enter);
                }
                Key::Named(NamedKey::Backspace) if ctrl_down => {
                    if !editor.delete_selection() {
                        let cursor = editor.cursor();
                        editor.set_selection(Selection::Normal(cursor));
                        editor.action(Action::Motion(Motion::PreviousWord));
                        editor.delete_selection();
                        editor.set_selection(Selection::None);
                    }
                }
                Key::Named(NamedKey::Delete) if ctrl_down => {
                    if !editor.delete_selection() {
                        let cursor = editor.cursor();
                        editor.set_selection(Selection::Normal(cursor));
                        editor.action(Action::Motion(Motion::NextWord));
                        editor.delete_selection();
                        editor.set_selection(Selection::None);
                    }
                    let cursor = editor.cursor();
                    editor.set_selection(Selection::Normal(cursor));
                    editor.action(Action::Motion(Motion::NextWord));
                    editor.delete_selection();
                    editor.set_selection(Selection::None);
                }
                Key::Named(NamedKey::Backspace) => {
                    if !editor.delete_selection() {
                        editor.action(Action::Backspace);
                    }
                }
                Key::Named(NamedKey::Delete) => {
                    if !editor.delete_selection() {
                        editor.action(Action::Delete);
                    }
                }
                Key::Named(key) => {
                    if let Some(text) = key.to_text() {
                        editor.delete_selection();
                        for c in text.chars() {
                            editor.action(Action::Insert(c));
                        }
                    }
                }
                Key::Character(text) => {
                    editor.delete_selection();
                    for c in text.chars() {
                        editor.action(Action::Insert(c));
                    }
                }
                _ => {}
            }

            drop(font_system);

            fetched.redraw = true;
        });

        let interface = world.single::<Interface>().unwrap();

        let tracker = world.observer(interface, move |Redraw, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if this.redraw {
                this.redraw();
            }
        });

        world.dependency(tracker, this);
    }
}

impl TextEdit {
    pub fn new(
        rect: Rectangle,
        text: String,
        manager: &mut TextManager,
        interface: &mut Interface,
    ) -> TextEdit {
        let mut font_system = manager.font_system.lock();
        let mut swash_cache = manager.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut font_system);

        let attrs = Attrs::new().family(Family::Monospace);
        buffer_borrow.set_size(Some(rect.width() as f32), Some(rect.height() as f32));
        buffer_borrow.set_text(&text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);

        let mut data = vec![0; (rect.width() * rect.height() * 4) as usize];

        buffer_borrow.draw(
            &mut swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * rect.width() as i32) * 4) as usize;
                let rgba = color.as_rgba();
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        let inner = Painter::new_with(rect, data, interface);

        TextEdit {
            inner,
            editor: Editor::new(buffer),
            font_system: manager.font_system.clone(),
            swash_cache: manager.swash_cache.clone(),
            redraw: false,
        }
    }

    pub fn clone_text(&self) -> String {
        self.editor.with_buffer(|buffer| {
            let mut selection = String::new();

            if let Some(line) = buffer.lines.first() {
                selection.push_str(line.text());
                for line_i in 1..buffer.lines.len() {
                    selection.push('\n');
                    selection.push_str(buffer.lines[line_i].text());
                }
            }

            selection
        })
    }

    fn redraw(&mut self) {
        self.redraw = false;

        let mut font_system = self.font_system.lock();
        let mut swash_cache = self.swash_cache.lock();

        let mut writer = self.inner.open_writer();
        writer.clear([0; 4]);
        self.editor.shape_as_needed(&mut font_system, true);
        self.editor.draw(
            &mut font_system,
            &mut swash_cache,
            Color::rgba(255, 255, 255, 255),
            Color::rgba(255, 255, 255, 127),
            Color::rgba(127, 127, 255, 127),
            Color::rgba(255, 255, 255, 255),
            |x, y, w, h, color| {
                let rgba = color.as_rgba();
                for x in x..(x + w as i32) {
                    for y in y..(y + h as i32) {
                        writer.draw(x, y, rgba);
                    }
                }
            },
        );
    }
}
