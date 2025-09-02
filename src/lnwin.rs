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
    elements::{Image, Label, Palette, PaletteKnob},
    interface::Interface,
    layout::{select::Selector, stroke::StrokeManager, world::World},
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
        let camera = CameraMove::default();

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

                // The viewport is updated before the viewport transform
                if self.camera.active {
                    let dx = self.camera.start_cursor[0] - screen[0];
                    let dy = self.camera.start_cursor[1] - screen[1];
                    let [dx, dy] = self.interface.screen_to_world_relative([dx, dy]);

                    self.interface.set_camera([
                        self.camera.camera_orig[0] + dx,
                        self.camera.camera_orig[1] + dy,
                    ]);

                    self.window.request_redraw();
                }

                let point = self.interface.screen_to_world(screen);
                self.cursor = screen;

                match self.state {
                    ActivatedTool::Selection => {
                        self.selector.cursor_position(point, &mut self.world);
                        self.window.request_redraw();
                    }
                    ActivatedTool::Stroke => {
                        self.stroke.cursor_position(point, &mut self.interface);
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
                    self.selector.cursor_pressed(&mut self.world);
                    self.window.request_redraw();
                }
                ActivatedTool::Stroke => {
                    self.stroke.cursor_pressed([0xff; 4], &mut self.interface);
                    self.window.request_redraw();
                }
            },
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => match self.state {
                ActivatedTool::Selection => {
                    self.selector.cursor_released();
                }
                ActivatedTool::Stroke => {
                    self.stroke.cursor_released();
                    self.window.request_redraw();
                }
            },
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera.active = true;
                self.camera.start_cursor = self.cursor;
                self.camera.camera_orig = self.interface.get_camera();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera.active = false;
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
                    let palette = self.world.insert(Palette::new([0, 0], &mut self.interface));
                    let knob = self.world.insert(PaletteKnob::new(palette, &mut self.interface));
                    let palette = self.world.fetch_mut::<Palette>(palette).unwrap();
                    palette.set_knob(knob);
                }
                _ => (),
            },

            WindowEvent::DroppedFile(path) => match Image::new(path, &mut self.interface) {
                Ok(image) => {
                    self.world.insert(image);
                }
                Err(err) => {
                    log::warn!("Drop File: {err}");
                }
            },

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
        if let ActivatedTool::Stroke = self.state {
            self.stroke.cursor_released();
        }
        self.state = tool;
        if let ActivatedTool::Stroke = self.state {
            self.stroke.update_color(&mut self.world);
        }
    }
}

#[derive(Default)]
struct CameraMove {
    start_cursor: [f64; 2],
    camera_orig: [i32; 2],
    active: bool,
}

#[derive(Clone, Copy)]
enum ActivatedTool {
    Stroke,
    Selection,
}
