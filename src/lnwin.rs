use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, Modifiers, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    elements::stroke::StrokeLayer,
    interface::{Interface, Redraw},
    measures::{DeltaFract, Fract, Position, PositionFract, Size},
    text::TextManager,
    tools::{focus::Focus, pointer::Pointer},
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct Lnwin {
    world: World,
}

impl ApplicationHandler for Lnwin {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.world.single::<Lnwindow>().is_none() {
            let lnwindow = pollster::block_on(Lnwindow::new(event_loop, &mut self.world));
            self.world.insert(lnwindow);
            self.world.flush();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match self.world.single::<Lnwindow>() {
            Some(window) => self.world.trigger(window, event),
            None => event_loop.exit(),
        }

        self.world.flush();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.world = World::default();
    }
}

/// The main window.
pub struct Lnwindow {
    window: Arc<Window>,
    viewport: Viewport,

    // Screen-space
    cursor: [f64; 2],

    camera_cursor_start: [f64; 2],
    camera_origin: Option<PositionFract>,
}

impl Element for Lnwindow {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |event: &WindowEvent, world, this| {
            let mut lnwindow = world.fetch_mut(this).unwrap();
            lnwindow.window_event(event, world, this);
        });

        world.insert(TextManager::default());
        world.insert(LnwinModifiers::default());
        world.insert(Focus::default());
        world.insert(StrokeLayer::default());
        world.insert(Pointer);
    }
}

impl Lnwindow {
    async fn new(event_loop: &ActiveEventLoop, world: &mut World) -> Lnwindow {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let size = window.inner_size();
        let viewport = Viewport {
            size: Size::new(size.width.max(1), size.height.max(1)),
            center: PositionFract::new(0, 0, 0, 0),
            zoom: Fract::new(0, 0),
        };

        let interface = Interface::new(window.clone(), &viewport).await;

        world.insert(interface);

        Lnwindow {
            window,
            viewport,
            cursor: [0.0, 0.0],
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
        }
    }

    fn window_event(&mut self, event: &WindowEvent, world: &World, this: Handle<Lnwindow>) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                // The viewport needs to be updated before the viewport transform
                self.cursor = self.viewport.cursor_to_screen(*position);
                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let delta = self.viewport.screen_to_world_relative([dx, dy]);

                    self.viewport.center = *camera_orig + delta;
                    self.window.request_redraw();
                }

                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Moved(point.floor()));

                self.window.request_redraw();
            }

            // Major Interaction //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Pressed(point.floor()));

                self.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Released(point.floor()));

                self.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerAltEvent(point.floor()));

                self.window.request_redraw();
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                let mut fetched = world.single_fetch_mut::<LnwinModifiers>().unwrap();
                fetched.0 = *modifiers;
            }

            WindowEvent::KeyboardInput { .. } => {
                self.window.request_redraw();
            }

            // Camera Move //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera_cursor_start = self.cursor;
                self.camera_origin = Some(self.viewport.center);
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
                        self.viewport.zoom += level;
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let level = delta.y.div_euclid(16.0) as i32 + 1;
                        self.viewport.zoom += level;
                    }
                }
                self.window.request_redraw();
            }

            // Render //
            WindowEvent::RedrawRequested => {
                let interface = world.single::<Interface>().unwrap();
                let mut fetched = world.fetch_mut(interface).unwrap();
                fetched.resize(&self.viewport);
                world.trigger(world.single::<Interface>().unwrap(), Redraw);
                world.queue(move |world| {
                    let mut fetched = world.fetch_mut(interface).unwrap();
                    fetched.restructure();
                    fetched.redraw();
                });
            }

            WindowEvent::Resized(size) => {
                self.viewport.size.w = size.width.max(1);
                self.viewport.size.h = size.height.max(1);
                self.window.request_redraw();
            }

            WindowEvent::CloseRequested => {
                world.remove(this);
            }

            _ => (),
        }
    }
}

pub struct Viewport {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,
}

impl Viewport {
    pub fn cursor_to_screen(&self, cursor: PhysicalPosition<f64>) -> [f64; 2] {
        let x = (cursor.x * 2.0) / self.size.w as f64 - 1.0;
        let y = 1.0 - (cursor.y * 2.0) / self.size.h as f64;
        [x, y]
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> PositionFract {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> DeltaFract {
        let scale = f64::powf(2.0, self.zoom.n as f64 + self.zoom.nf as f64 * 1e-32);
        let x = delta[0] / scale * self.size.w as f64 / 2.0;
        let y = delta[1] / scale * self.size.h as f64 / 2.0;
        DeltaFract::new(
            x.floor() as i32,
            (((x - x.floor()) * 32f64.exp2()).floor()) as u32,
            y.floor() as i32,
            (((y - y.floor()) * 32f64.exp2()).floor()) as u32,
        )
    }
}

/// Pointer that has been transformed into world-space
#[derive(Debug, Clone, Copy)]
pub enum PointerEvent {
    Moved(Position),
    Pressed(Position),
    Released(Position),
}

#[derive(Debug, Clone, Copy)]
pub struct PointerAltEvent(pub Position);

#[derive(Default)]
pub struct LnwinModifiers(pub Modifiers);
impl Element for LnwinModifiers {}
