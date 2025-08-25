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
    elements::Image, interface::Interface, layout::{select::Selector, stroke::StrokeManager, world::World}
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

    state: ActivatedTool,

    world: World,
    selector: Selector,
    stroke: StrokeManager,
    camera: CameraMove,
}
impl Lnwindow {
    pub async fn new(event_loop: &ActiveEventLoop) -> Lnwindow {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let mut interface = Interface::new(window.clone(), width, height).await;

        let cursor = [0.0, 0.0];
        let state = ActivatedTool::Stroke;

        let world = World::new();
        let selector = Selector::new(&mut interface);
        let stroke = StrokeManager::new();
        let camera = CameraMove::new(state);

        Lnwindow {
            window,
            interface,
            width,
            height,
            cursor,
            state,
            world,
            selector,
            stroke,
            camera,
        }
    }

    pub fn window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let screen = self.cursor_to_screen(position);
                let point = self.interface.screen_to_world(screen);
                self.cursor = screen;
                match self.state {
                    ActivatedTool::Selection => {
                        self.selector.cursor_position(point, &self.world);
                        self.window.request_redraw();
                    }
                    ActivatedTool::Stroke => {
                        self.stroke.cursor_position(point, &mut self.interface);
                        self.window.request_redraw();
                    }
                    ActivatedTool::Move => {
                        let dx = self.camera.start_cursor[0] - self.cursor[0];
                        let dy = self.camera.start_cursor[1] - self.cursor[1];
                        let [dx, dy] = self.interface.screen_to_world_relative([dx, dy]);

                        self.interface.set_camera([
                            self.camera.camera_orig[0] + dx,
                            self.camera.camera_orig[1] + dy,
                        ]);

                        self.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => match self.state {
                ActivatedTool::Selection => {
                    self.selector.cursor_click(&mut self.world);
                    self.window.request_redraw();
                }
                ActivatedTool::Stroke => {
                    self.stroke.cursor_pressed([0xff; 4], &mut self.interface);
                    self.window.request_redraw();
                }
                _ => (),
            },
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => match self.state {
                ActivatedTool::Selection => {}
                ActivatedTool::Stroke => {
                    self.stroke.cursor_released();
                    self.window.request_redraw();
                }
                _ => (),
            },
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera.prev_tool = self.state;
                self.switch_tool(ActivatedTool::Move);
                self.camera.start_cursor = self.cursor;
                self.camera.camera_orig = self.interface.get_camera();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                self.switch_tool(self.camera.prev_tool);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::LineDelta(_rows, lines) => {
                        let level = lines.ceil() as i32;
                        let factor = f32::powi(2.0, level);
                        let zoom = self.interface.get_zoom();
                        self.interface.set_zoom(zoom * factor);
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let level = delta.y.div_euclid(16.0) as i32 + 1;
                        let factor = f32::powi(2.0, level);
                        let zoom = self.interface.get_zoom();
                        self.interface.set_zoom(zoom * factor);
                    }
                }
                self.window.request_redraw();
            }
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
                KeyCode::KeyB => {
                    self.switch_tool(ActivatedTool::Stroke);
                }
                KeyCode::KeyS => {
                    self.switch_tool(ActivatedTool::Selection);
                }
                _ => (),
            },

            WindowEvent::DroppedFile(path) => {
                match Image::new(path, &mut self.interface) {
                    Ok(image) => {
                        self.world.insert(image);
                    }
                    Err(err) => {
                        log::warn!("Drop File: {err}");
                    }
                }
            }

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

    fn switch_tool(&mut self, tool: ActivatedTool) {
        // Final for tools
        match self.state {
            ActivatedTool::Selection => {
                self.selector.stop();
            }
            ActivatedTool::Stroke => {
                self.stroke.cursor_released();
            }
            _ => (),
        }
        self.state = tool;
    }
}

struct CameraMove {
    start_cursor: [f64; 2],
    camera_orig: [i32; 2],
    prev_tool: ActivatedTool,
}
impl CameraMove {
    fn new(prev_tool: ActivatedTool) -> Self {
        CameraMove {
            start_cursor: [0.0, 0.0],
            camera_orig: [0, 0],
            prev_tool,
        }
    }
}

#[derive(Clone, Copy)]
enum ActivatedTool {
    Stroke,
    Selection,
    Move,
}
