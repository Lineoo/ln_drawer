use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, Modifiers, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    elements::{Image, Menu, StrokeLayer},
    interface::{Interface, Redraw},
    measures::Position,
    text::TextManager,
    tools::{focus::Focus, pointer::Pointer},
    world::{Element, InsertElement, World, WorldCellEntry},
};

#[derive(Default)]
pub struct Lnwin {
    world: World,
}
impl ApplicationHandler for Lnwin {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.world.single::<Lnwindow>().is_none() {
            let lnwindow = pollster::block_on(Lnwindow::new(event_loop, &mut self.world));
            self.world.insert(lnwindow);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match self.world.single_entry::<Lnwindow>() {
            Some(mut window) => window.trigger(&event),
            None => event_loop.exit(),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.world = World::default();
    }
}

/// The main window.
pub struct Lnwindow {
    window: Arc<Window>,
    viewport: Viewport,

    // Screen-space
    cursor: [f64; 2],

    camera_cursor_start: [f64; 2],
    camera_origin: Option<[i32; 2]>,
}
impl Element for Lnwindow {}
impl InsertElement for Lnwindow {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<WindowEvent>(|event, entry| {
            let mut lnwindow = entry.fetch_mut().unwrap();
            let entry = entry.entry(entry.handle()).unwrap();
            lnwindow.window_event(event, entry);
        });

        entry.insert(TextManager::default());
        entry.insert(LnwinModifiers::default());
        entry.insert(Focus::default());
        let stroke = entry.insert(StrokeLayer::default()).untyped();
        let mut selection = Pointer::default();
        selection.set_fallback(stroke);
        entry.insert(selection);
    }
}
impl Lnwindow {
    async fn new(event_loop: &ActiveEventLoop, world: &mut World) -> Lnwindow {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let size = window.inner_size();
        let viewport = Viewport {
            width: size.width.max(1),
            height: size.height.max(1),
            camera: [0, 0],
            zoom: 0,
        };
        let interface = Interface::new(window.clone(), &viewport).await;

        world.insert(interface);

        Lnwindow {
            window,
            viewport,
            cursor: [0.0, 0.0],
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
        }
    }

    fn window_event(&mut self, event: &WindowEvent, entry: WorldCellEntry<Lnwindow>) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // The viewport needs to be updated before the viewport transform
                self.cursor = self.viewport.cursor_to_screen(*position);
                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let [dx, dy] = self.viewport.screen_to_world_relative([dx, dy]);

                    self.viewport.camera = [camera_orig[0] + dx, camera_orig[1] + dy];
                    self.window.request_redraw();
                }

                let point = self.viewport.screen_to_world(self.cursor);
                entry.trigger(PointerEvent::Moved(point));

                self.window.request_redraw();
            }

            // Major Interaction //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                entry.trigger(PointerEvent::Pressed(point));
                self.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                entry.trigger(PointerEvent::Released(point));
                self.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                if let Some(menu) = entry.single_entry::<Menu>() {
                    menu.destroy();
                }
                entry.insert(Menu::new(
                    point,
                    &mut entry.single_fetch_mut().unwrap(),
                    &mut entry.single_fetch_mut().unwrap(),
                ));
                self.window.request_redraw();
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                entry.single_fetch_mut::<LnwinModifiers>().unwrap().0 = *modifiers;
            }

            WindowEvent::KeyboardInput { .. } => {
                self.window.request_redraw();
            }

            // Camera Move //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera_cursor_start = self.cursor;
                self.camera_origin = Some(self.viewport.camera);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera_origin = None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::LineDelta(_rows, lines) => {
                        let level = lines.ceil() as i32;
                        self.viewport.zoom += level;
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let level = delta.y.div_euclid(16.0) as i32 + 1;
                        self.viewport.zoom += level;
                    }
                }
                self.window.request_redraw();
            }

            // Misc //
            WindowEvent::DroppedFile(path) => {
                match Image::new(path, &mut entry.single_fetch_mut().unwrap()) {
                    Ok(image) => {
                        entry.insert(image);
                    }
                    Err(err) => {
                        log::warn!("Drop File: {err}");
                    }
                }
            }

            // Render //
            WindowEvent::RedrawRequested => {
                let mut interface = entry.single_fetch_mut::<Interface>().unwrap();
                interface.resize(&self.viewport);
                entry.single_entry::<Interface>().unwrap().trigger(&Redraw);
                interface.restructure();
                interface.redraw();
            }
            WindowEvent::Resized(size) => {
                self.viewport.width = size.width.max(1);
                self.viewport.height = size.height.max(1);
                self.window.request_redraw();
            }

            WindowEvent::CloseRequested => {
                entry.destroy();
            }

            _ => (),
        }
    }
}

pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub camera: [i32; 2],
    pub zoom: i32,
}
impl Viewport {
    pub fn cursor_to_screen(&self, cursor: PhysicalPosition<f64>) -> [f64; 2] {
        let x = (cursor.x * 2.0) / self.width as f64 - 1.0;
        let y = 1.0 - (cursor.y * 2.0) / self.height as f64;
        [x, y]
    }

    pub fn world_to_screen(&self, point: Position) -> [f64; 2] {
        let x = (point.x - self.camera[0]) as f64 / self.width as f64 * 2.0;
        let y = (point.x - self.camera[1]) as f64 / self.height as f64 * 2.0;
        let scale = f64::powi(2.0, self.zoom);
        [x * scale, y * scale]
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> Position {
        let scale = f64::powi(2.0, self.zoom);
        let x = (point[0] / scale * self.width as f64 / 2.0).floor() as i32 + self.camera[0];
        let y = (point[1] / scale * self.height as f64 / 2.0).floor() as i32 + self.camera[1];
        Position::new(x, y)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> [i32; 2] {
        let scale = f64::powi(2.0, self.zoom);
        let x = (delta[0] / scale * self.width as f64 / 2.0).floor() as i32;
        let y = (delta[1] / scale * self.height as f64 / 2.0).floor() as i32;
        [x, y]
    }
}

/// Pointer that has been transformed into world-space
#[derive(Clone, Copy)]
pub enum PointerEvent {
    Moved(Position),
    Pressed(Position),
    Released(Position),
}

#[derive(Default)]
pub struct LnwinModifiers(pub Modifiers);
impl Element for LnwinModifiers {}
impl InsertElement for LnwinModifiers {}
