use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use octotablet::events::{Event as OctoEvent, ToolEvent};

use crate::{
    elements::{ButtonRaw, Image, Label, StrokeLayer},
    interface::{Interface, Wireframe},
    layout::world::{ElementHandle, World},
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

    image: Option<Image>,
    world: Option<World>,
    selection_wireframe: Option<Wireframe>,
    stroke: Option<ElementHandle>,

    manager: Option<octotablet::Manager>,
    tablet_down: bool,
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

                    let stroke = self
                        .world
                        .as_mut()
                        .unwrap()
                        .fetch_mut::<StrokeLayer>(self.stroke.unwrap())
                        .unwrap();

                    stroke.write_pixel([x, y], [15, 230, 255, 255], renderer);

                    renderer.restructure();
                    renderer.redraw();
                }

                if let Some(renderer) = &mut self.renderer
                    && self.right_button_down
                {
                    let x = -(delta[0] * 2.0) / self.width as f64;
                    let y = (delta[1] * 2.0) / self.height as f64;
                    let [x, y] = renderer.screen_to_world_relative([x, y]);

                    let position = renderer.get_camera();
                    renderer.set_camera([position[0] + x, position[1] + y]);
                    renderer.restructure();
                    renderer.redraw();
                }

                if let Some(world) = &mut self.world
                    && let Some(renderer) = &mut self.renderer
                {
                    let x = (self.cursor_position.x * 2.0) / self.width as f64 - 1.0;
                    let y = 1.0 - (self.cursor_position.y * 2.0) / self.height as f64;
                    let [x, y] = renderer.screen_to_world([x, y]);

                    if let Some(element) = world.intersect(x, y) {
                        let element = world.fetch_mut_dyn(element).unwrap();
                        self.selection_wireframe =
                            Some(renderer.create_wireframe(element.border(), [1.0, 0.0, 0.0, 1.0]));
                    } else {
                        self.selection_wireframe = None;
                    }
                    renderer.restructure();
                    renderer.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left
                    && let Some(renderer) = &mut self.renderer
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
                    } else if state == ElementState::Released {
                        self.cursor_wireframe = None;
                    }

                    if let Some(world) = &mut self.world {
                        let x = (self.cursor_position.x * 2.0) / self.width as f64 - 1.0;
                        let y = 1.0 - (self.cursor_position.y * 2.0) / self.height as f64;
                        let [x, y] = renderer.screen_to_world([x, y]);

                        if let Some(element) = world.intersect_with::<ButtonRaw>(x, y) {
                            let button = world.fetch_mut::<ButtonRaw>(element).unwrap();
                            button.pressed();
                        } else {
                            self.selection_wireframe = None;
                        }
                        renderer.restructure();
                        renderer.redraw();
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
                    let zoom = renderer.get_zoom();
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
                        PhysicalKey::Code(KeyCode::Equal) => {
                            renderer.set_zoom(zoom * 2.0);
                        }
                        PhysicalKey::Code(KeyCode::Minus) => {
                            renderer.set_zoom(zoom * 0.5);
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
                    renderer.resize(self.width, self.height);
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(manager) = &mut self.manager {
            for event in manager.pump().into_iter().flatten() {
                log::trace!("{event:?}");
                #[expect(clippy::single_match)]
                match event {
                    OctoEvent::Tool { tool, event } => match event {
                        ToolEvent::Down => self.tablet_down = true,
                        ToolEvent::Up | ToolEvent::Out => self.tablet_down = false,
                        ToolEvent::Pose(pose) => {
                            if self.tablet_down
                                && let Some(renderer) = &mut self.renderer
                                && let Some(world) = &mut self.world
                            {
                                let x = (pose.position[0] as f64 * 2.0) / self.width as f64 - 1.0;
                                let y = 1.0 - (pose.position[1] as f64 * 2.0) / self.height as f64;
                                let [x, y] = renderer.screen_to_world([x, y]);

                                let stroke = self
                                    .world
                                    .as_mut()
                                    .unwrap()
                                    .fetch_mut::<StrokeLayer>(self.stroke.unwrap())
                                    .unwrap();

                                stroke.write_pixel([x, y], [15, 230, 255, 255], renderer);

                                renderer.restructure();
                                renderer.redraw();
                            }
                        }
                        _ => (),
                    },
                    _ => (),
                }
            }
        }
    }
}

impl LnDrawer {
    fn init_window(&mut self, event_loop: &ActiveEventLoop) {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let manager = octotablet::Builder::new().build_shared(&window).unwrap();
        self.manager = Some(manager);

        let size = window.inner_size();
        self.width = size.width;
        self.height = size.height;

        let mut renderer =
            pollster::block_on(Interface::new(window.clone(), self.width, self.height));

        let mut world = World::new();
        world.insert(Image::from_bytes(include_bytes!("../res/icon.png"), &mut renderer).unwrap());
        world.insert(ButtonRaw::new([-100, -100, 0, 0], || {
            println!("Hi there!");
        }));
        world.insert(Label::new(
            [0, 0, 300, 100],
            "Hello, LnDrawer!".into(),
            &mut renderer,
        ));
        self.stroke = Some(world.insert(StrokeLayer::new()));
        self.world = Some(world);

        self.window = Some(window);
        self.renderer = Some(renderer);
    }
}
