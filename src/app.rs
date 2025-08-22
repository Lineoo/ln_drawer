use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
    elements::Image,
    interface::{Interface, Painter, Wireframe},
    layout::world::World,
};

#[derive(Default)]
pub struct LnDrawer {
    window: Option<Arc<Window>>,
    renderer: Option<Interface>,

    cursor_start: PhysicalPosition<f64>,
    cursor_position: PhysicalPosition<f64>,
    cursor_wireframe: Option<Wireframe>,

    width: u32,
    height: u32,

    right_button_down: bool,

    painter: Option<Painter>,
    image: Option<Image>,
    world: Option<World>,
    selection_wireframe: Option<Wireframe>,
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
                self.image = None;
                self.world = None;
                self.selection_wireframe = None;

                event_loop.exit();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let delta = [
                    position.x - self.cursor_position.x,
                    position.y - self.cursor_position.y,
                ];

                self.cursor_position = position;
                if let Some(wireframe) = &mut self.cursor_wireframe
                    && let Some(painter) = &mut self.painter
                    && let Some(renderer) = &mut self.renderer
                {
                    let sx = (self.cursor_start.x * 2.0) / self.width as f64 - 1.0;
                    let sy = 1.0 - (self.cursor_start.y * 2.0) / self.height as f64;
                    let [sx, sy] = renderer.screen_to_world([sx, sy]);

                    let x = (self.cursor_position.x * 2.0) / self.width as f64 - 1.0;
                    let y = 1.0 - (self.cursor_position.y * 2.0) / self.height as f64;
                    let [x, y] = renderer.screen_to_world([x, y]);

                    wireframe.set_rect([sx, sy, x, y]);
                    wireframe.set_color([
                        ((self.cursor_position.x - self.cursor_start.x).abs() as f32 * 0.001)
                            .clamp(0.0, 1.0),
                        ((self.cursor_position.y - self.cursor_start.y).abs() as f32 * 0.001)
                            .clamp(0.0, 1.0),
                        0.5,
                        1.0,
                    ]);

                    painter.set_pixel(
                        (self.cursor_position.x - self.width as f64 * 0.5).floor() as i32,
                        (self.height as f64 * 0.5 - self.cursor_position.y).floor() as i32,
                        [85, 145, 255, 255],
                    );

                    renderer.restructure();
                    renderer.redraw();
                }

                if let Some(renderer) = &mut self.renderer
                    && self.right_button_down
                {
                    let position = renderer.get_camera();
                    renderer
                        .set_camera([position[0] - delta[0] as i32, position[1] + delta[1] as i32]);
                    renderer.restructure();
                    renderer.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left
                    && let Some(renderer) = &mut self.renderer
                    && let Some(world) = &mut self.world
                {
                    if state == ElementState::Pressed {
                        self.cursor_start = self.cursor_position;
                        self.cursor_wireframe = Some(renderer.create_wireframe(
                            [
                                (self.cursor_position.x - self.width as f64 * 0.5).floor() as i32,
                                (self.height as f64 * 0.5 - self.cursor_position.y).floor() as i32,
                                (self.cursor_position.x - self.width as f64 * 0.5).floor() as i32,
                                (self.height as f64 * 0.5 - self.cursor_position.y).floor() as i32,
                            ],
                            [1.0, 0.0, 0.0, 1.0],
                        ));
                        if let Some(element) = world.intersect(
                            (self.cursor_position.x - self.width as f64 * 0.5).floor() as i32,
                            (self.height as f64 * 0.5 - self.cursor_position.y).floor() as i32,
                        ) {
                            let element = world.fetch_dyn(element).unwrap();
                            self.selection_wireframe = Some(
                                renderer.create_wireframe(element.border(), [1.0, 0.0, 0.0, 1.0]),
                            );
                        } else {
                            self.selection_wireframe = None;
                        }
                    } else if state == ElementState::Released {
                        self.cursor_wireframe = None;
                    }
                    renderer.restructure();
                    renderer.redraw();
                }
                if button == MouseButton::Right {
                    self.right_button_down = state == ElementState::Pressed;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.restructure();
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
                    renderer.restructure();
                    renderer.redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    self.width = size.width.max(1);
                    self.height = size.height.max(1);
                    renderer.resize(size);
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

        self.painter = Some(renderer.create_painter([-400, -300, 400, 300]));

        let size = window.inner_size();
        self.width = size.width;
        self.height = size.height;

        let mut world = World::new();
        world.insert(Image::from_bytes(include_bytes!("../res/icon.png"), &mut renderer).unwrap());
        self.world = Some(world);

        self.window = Some(window);
        self.renderer = Some(renderer);
    }
}
