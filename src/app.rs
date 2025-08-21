use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::interface::{Interface, Painter, Wireframe};

#[derive(Default)]
pub struct LnDrawer {
    window: Option<Arc<Window>>,
    renderer: Option<Interface>,

    cursor_start: PhysicalPosition<f64>,
    cursor_position: PhysicalPosition<f64>,
    cursor_wireframe: Option<Arc<Wireframe>>,

    right_button_down: bool,

    painter: Option<Arc<Painter>>,
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
                self.cursor_wireframe = None;
                self.painter = None;

                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let delta = [
                    position.x - self.cursor_position.x,
                    position.y - self.cursor_position.y,
                ];

                self.cursor_position = position;
                if let Some(wireframe) = &self.cursor_wireframe
                    && let Some(painter) = &self.painter
                    && let Some(renderer) = &mut self.renderer
                {
                    let screen = cursor_to_screen(self.cursor_position, renderer);
                    let screen_start = cursor_to_screen(self.cursor_start, renderer);
                    wireframe.set_rect(
                        [screen_start.0, screen_start.1, screen.0, screen.1],
                        renderer.queue(),
                    );
                    wireframe.set_color(
                        [
                            (screen_start.0 - screen.0).abs().clamp(0.0, 1.0),
                            (screen_start.1 - screen.1).abs().clamp(0.0, 1.0),
                            0.5,
                            1.0,
                        ],
                        renderer.queue(),
                    );

                    painter.set_pixel(
                        (self.cursor_position.x as f32).rem_euclid(renderer.width() as f32) as u32,
                        (renderer.height() as f32 - self.cursor_position.y as f32)
                            .rem_euclid(renderer.height() as f32) as u32,
                        [85, 145, 255, 255],
                    );
                    painter.flush(renderer.queue());

                    renderer.redraw();
                }
                
                if let Some(renderer) = &mut self.renderer
                    && self.right_button_down
                {
                    let position = renderer.get_camera();
                    renderer
                        .set_camera([position[0] - delta[0] as i32, position[1] + delta[1] as i32]);
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
                        self.cursor_wireframe = Some(renderer.create_wireframe(
                            [screen.0, screen.1, screen.0, screen.1],
                            [1.0, 0.0, 0.0, 1.0],
                        ));
                    } else if state == ElementState::Released {
                        self.cursor_wireframe = None;
                    }
                    renderer.redraw();
                }
                if button == MouseButton::Right {
                    self.right_button_down = state == ElementState::Pressed;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(renderer) = &mut self.renderer
                    && event.state == ElementState::Pressed
                {
                    let camera = renderer.get_camera();
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            renderer.set_camera([camera[0] + 1, camera[1]]);
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            renderer.set_camera([camera[0], camera[1] - 1]);
                        }
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            renderer.set_camera([camera[0] - 1, camera[1]]);
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            renderer.set_camera([camera[0], camera[1] + 1]);
                        }
                        _ => (),
                    }
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

        let mut renderer = pollster::block_on(Interface::new(window.clone()));

        self.painter = Some(renderer.create_painter(
            [-1.0, -1.0, 1.0, 1.0],
            renderer.width(),
            renderer.height(),
        ));

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
