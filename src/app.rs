use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::interface::{Interface, Wireframe};

#[derive(Default)]
pub struct LnDrawer {
    window: Option<Arc<Window>>,
    renderer: Option<Interface>,

    cursor_start: PhysicalPosition<f64>,
    cursor_position: PhysicalPosition<f64>,
    cursor_wireframe: Option<Arc<Wireframe>>,
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
                if let Some(wireframe) = &self.cursor_wireframe
                    && let Some(renderer) = &mut self.renderer
                {
                    let screen = cursor_to_screen(self.cursor_position, renderer);
                    let screen_start = cursor_to_screen(self.cursor_start, renderer);
                    wireframe.set_rect(
                        [screen_start.0, screen_start.1, screen.0, screen.1],
                        renderer.queue(),
                    );
                    renderer.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left
                    && let Some(renderer) = &mut self.renderer
                {
                    if state == ElementState::Pressed {
                        self.cursor_start = self.cursor_position;
                        let screen = cursor_to_screen(self.cursor_position, renderer);
                        self.cursor_wireframe = Some(renderer.create_wireframe_instance(
                            [screen.0, screen.1, screen.0, screen.1],
                            [1.0, 0.0, 0.0, 1.0],
                        ));
                    } else if state == ElementState::Released {
                        self.cursor_wireframe = None;
                    }
                    renderer.redraw();
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

fn cursor_to_screen(cursor: PhysicalPosition<f64>, renderer: &Interface) -> (f32, f32) {
    (
        cursor.x as f32 / renderer.width() as f32 * 2.0 - 1.0,
        1.0 - cursor.y as f32 / renderer.height() as f32 * 2.0,
    )
}
