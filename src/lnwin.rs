use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
    elements::{Image, Label, Palette, StrokeLayer},
    interface::Interface,
    world::World,
};

#[derive(Default)]
pub struct Lnwin {
    window: Option<Lnwindow>,
}
impl ApplicationHandler for Lnwin {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let lnwindow = pollster::block_on(Lnwindow::new(event_loop));
            self.window = Some(lnwindow);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            self.window = None;
            event_loop.exit();
            return;
        }

        if let Some(window) = &mut self.window {
            window.window_event(event);
        }
    }
}

/// The main window.
struct Lnwindow {
    window: Arc<Window>,
    interface: Interface,

    width: u32,
    height: u32,

    cursor: [f64; 2],

    camera_cursor_start: [f64; 2],
    camera_origin: Option<[i32; 2]>,

    world: World,
}
impl Lnwindow {
    pub async fn new(event_loop: &ActiveEventLoop) -> Lnwindow {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let interface = Interface::new(window.clone(), width, height).await;

        Lnwindow {
            window,
            interface,
            width,
            height,
            cursor: [0.0, 0.0],
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
            world: World::default(),
        }
    }

    pub fn window_event(&mut self, event: WindowEvent) {
        self.world.trigger(&event);
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // The viewport needs to be updated before the viewport transform
                self.cursor = self.cursor_to_screen(position);
                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let [dx, dy] = self.interface.screen_to_world_relative([dx, dy]);

                    self.interface
                        .set_camera([camera_orig[0] + dx, camera_orig[1] + dy]);

                    self.window.request_redraw();
                }

                let point = self.interface.screen_to_world(self.cursor);
                self.world.trigger(&PointerEvent::Moved(point));

                self.window.request_redraw();
            }

            // Major Interaction //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.interface.screen_to_world(self.cursor);
                self.world.trigger(&PointerEvent::Pressed(point));
                self.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.interface.screen_to_world(self.cursor);
                self.world.trigger(&PointerEvent::Released(point));
                self.window.request_redraw();
            }

            // Camera Move //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera_cursor_start = self.cursor;
                self.camera_origin = Some(self.interface.get_camera());
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
                        let zoom = self.interface.get_zoom();
                        self.interface.set_zoom(zoom + level);
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let level = delta.y.div_euclid(16.0) as i32 + 1;
                        let zoom = self.interface.get_zoom();
                        self.interface.set_zoom(zoom + level);
                    }
                }
                self.window.request_redraw();
            }

            // Keyboard //
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => match keycode {
                KeyCode::F1 => {
                    self.world.insert(Label::new(
                        [0, 0, 200, 24],
                        "Hello, LnDrawer!".into(),
                        &mut self.interface,
                    ));
                    self.world.insert(
                        Image::from_bytes(include_bytes!("../res/icon.png"), &mut self.interface)
                            .unwrap(),
                    );
                }
                KeyCode::F2 => {
                    self.world.insert(Palette::new([0, 0], &mut self.interface));
                }
                KeyCode::F3 => {
                    self.world.insert(StrokeLayer::default());
                }
                _ => (),
            },

            // Misc //
            WindowEvent::DroppedFile(path) => match Image::new(path, &mut self.interface) {
                Ok(image) => {
                    self.world.insert(image);
                }
                Err(err) => {
                    log::warn!("Drop File: {err}");
                }
            },

            // Render //
            WindowEvent::RedrawRequested => {
                self.interface.restructure();
                self.interface.redraw();
            }
            WindowEvent::Resized(size) => {
                self.width = size.width.max(1);
                self.height = size.height.max(1);
                self.interface.resize(self.width, self.height);
            }

            _ => (),
        }
    }

    pub fn cursor_to_screen(&self, cursor: PhysicalPosition<f64>) -> [f64; 2] {
        let x = (cursor.x * 2.0) / self.width as f64 - 1.0;
        let y = 1.0 - (cursor.y * 2.0) / self.height as f64;
        [x, y]
    }
}

/// Pointer that has been transformed into world-space
pub enum PointerEvent {
    Moved([i32; 2]),
    Pressed([i32; 2]),
    Released([i32; 2]),
}
