use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{elements::StrokeLayer, interface::Interface, world::World};

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
    viewport: Viewport,
    world: World,

    // Screen-space
    cursor: [f64; 2],

    camera_cursor_start: [f64; 2],
    camera_origin: Option<[i32; 2]>,
}
impl Lnwindow {
    pub async fn new(event_loop: &ActiveEventLoop) -> Lnwindow {
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
        let interface = Interface::new(window.clone(), viewport.width, viewport.height).await;

        let mut world = World::default();
        world.insert(interface);

        Lnwindow {
            window,
            viewport,
            world,
            cursor: [0.0, 0.0],
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
        }
    }

    pub fn window_event(&mut self, event: WindowEvent) {
        self.world.trigger(&event);
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // The viewport needs to be updated before the viewport transform
                self.cursor = self.viewport.cursor_to_screen(position);
                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let [dx, dy] = self.viewport.screen_to_world_relative([dx, dy]);

                    {
                        let this = &mut self.viewport;
                        let position = [camera_orig[0] + dx, camera_orig[1] + dy];
                        this.camera = position;
                    };

                    self.window.request_redraw();
                }

                let point = self.viewport.screen_to_world(self.cursor);
                self.world.trigger(&PointerEvent::Moved(point));

                self.window.request_redraw();
            }

            // Major Interaction //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                self.world.trigger(&PointerEvent::Pressed(point));
                self.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
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
                self.camera_origin = Some({
                    let this = &self.viewport;
                    this.camera
                });
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
                        let zoom = {
                            let this = &self.viewport;
                            this.zoom
                        };
                        {
                            let this = &mut self.viewport;
                            let zoom = zoom + level;
                            this.zoom = zoom;
                        };
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let level = delta.y.div_euclid(16.0) as i32 + 1;
                        let zoom = {
                            let this = &self.viewport;
                            this.zoom
                        };
                        {
                            let this = &mut self.viewport;
                            let zoom = zoom + level;
                            this.zoom = zoom;
                        };
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
                    // self.world.insert(Label::new(
                    //     [0, 0, 200, 24],
                    //     "Hello, LnDrawer!".into(),
                    //     &mut self.interface,
                    // ));
                    // self.world.insert(
                    //     Image::from_bytes(include_bytes!("../res/icon.png"), &mut self.interface)
                    //         .unwrap(),
                    // );
                }
                KeyCode::F2 => {
                    // self.world.insert(Palette::new([0, 0], &mut self.interface));
                }
                KeyCode::F3 => {
                    self.world.insert(StrokeLayer::default());
                }
                _ => (),
            },

            // Render //
            WindowEvent::RedrawRequested => {
                let interface = self.world.single_mut::<Interface>().unwrap();
                interface.resize(&self.viewport);
                interface.restructure();
                interface.redraw();
            }
            WindowEvent::Resized(size) => {
                self.viewport.width = size.width.max(1);
                self.viewport.height = size.height.max(1);
                self.window.request_redraw();
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

    pub fn world_to_screen(&self, point: [i32; 2]) -> [f64; 2] {
        let x = (point[0] - self.camera[0]) as f64 / self.width as f64 * 2.0;
        let y = (point[1] - self.camera[1]) as f64 / self.height as f64 * 2.0;
        let scale = f64::powi(2.0, self.zoom);
        [x * scale, y * scale]
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> [i32; 2] {
        let scale = f64::powi(2.0, self.zoom);
        let x = (point[0] / scale * self.width as f64 / 2.0).floor() as i32 + self.camera[0];
        let y = (point[1] / scale * self.height as f64 / 2.0).floor() as i32 + self.camera[1];
        [x, y]
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> [i32; 2] {
        let scale = f64::powi(2.0, self.zoom);
        let x = (delta[0] / scale * self.width as f64 / 2.0).floor() as i32;
        let y = (delta[1] / scale * self.height as f64 / 2.0).floor() as i32;
        [x, y]
    }
}

/// Pointer that has been transformed into world-space
pub enum PointerEvent {
    Moved([i32; 2]),
    Pressed([i32; 2]),
    Released([i32; 2]),
}
