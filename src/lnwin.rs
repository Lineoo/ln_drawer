use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
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

    world: World,
    selector: Selector,
    stroke: StrokeManager,

    state: ActivatedTool,
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

        let world = World::new();
        let selector = Selector::new(&mut interface);
        let stroke = StrokeManager::new();

        let state = ActivatedTool::Selection;

        Lnwindow {
            window,
            interface,
            width,
            height,
            world,
            selector,
            stroke,
            state,
        }
    }

    pub fn window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let point = self.cursor_to_screen(position);
                let point = self.interface.screen_to_world(point);
                match self.state {
                    ActivatedTool::Selection => {
                        self.selector.cursor_position(point, &self.world);
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
                    self.selector.cursor_click(&mut self.world);
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
                ActivatedTool::Selection => {}
                ActivatedTool::Stroke => {
                    self.stroke.cursor_released();
                    self.window.request_redraw();
                }
            },
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
        self.state = tool;
        match tool {
            ActivatedTool::Selection => {
                log::info!("switch to selection");
                self.selector.stop();
            }
            ActivatedTool::Stroke => {
                log::info!("switch to stroke");
                self.stroke.cursor_released();
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ActivatedTool {
    Stroke,
    Selection,
}
