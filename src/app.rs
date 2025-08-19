use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::interface::Interface;

#[derive(Default)]
pub struct LnDrawer {
    window: Option<Arc<Window>>,
    renderer: Option<Interface>,

    cursor_position: PhysicalPosition<f64>,
    mouse_down: bool,
}

impl ApplicationHandler for LnDrawer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.init_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        log::trace!("{event:?}");
        match event {
            WindowEvent::CloseRequested => {
                self.window = None;
                self.renderer = None; // Drop or it will get seg fault

                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = position;
                if self.mouse_down
                    && let Some(renderer) = &mut self.renderer
                {
                    
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    if state == ElementState::Pressed {
                        self.mouse_down = true;
                    } else if state == ElementState::Released {
                        self.mouse_down = false;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.redraw();
                }
            }
            _ => (),
        }
    }
}

impl LnDrawer {
    fn init_window(&mut self, event_loop: &ActiveEventLoop) {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let renderer = pollster::block_on(Interface::new(window.clone()));

        self.window = Some(window);
        self.renderer = Some(renderer);
    }
}
