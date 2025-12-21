use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    elements::stroke::StrokeLayer,
    measures::{Fract, Position, PositionFract},
    render::{
        Render,
        canvas::CanvasManagerDescriptor,
        rounded::RoundedRectManagerDescriptor,
        text::TextManagerDescriptor,
        viewport::{Viewport, ViewportDescriptor, ViewportManagerDescriptor},
        wireframe::WireframeManagerDescriptor,
    },
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
        world.insert(Render::new(window.clone()).await);
        world.flush();

        world.insert(world.build(ViewportManagerDescriptor));
        world.flush();

        world.insert(world.build(ViewportDescriptor {
            size: [size.width.max(1), size.height.max(1)],
            ..Default::default()
        }));
        world.flush();

        world.insert(world.build(CanvasManagerDescriptor));
        world.insert(world.build(RoundedRectManagerDescriptor));
        world.insert(world.build(TextManagerDescriptor));
        world.insert(world.build(WireframeManagerDescriptor));
        world.flush();

        Lnwindow {
            window,
            cursor: [0.0, 0.0],
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
        }
    }

    fn window_event(&mut self, event: &WindowEvent, world: &World, this: Handle<Lnwindow>) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

                // The viewport needs to be updated before the viewport transform
                let size = self.window.inner_size();
                let x = (position.x * 2.0) / size.width as f64 - 1.0;
                let y = 1.0 - (position.y * 2.0) / size.height as f64;
                self.cursor = [x, y];

                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let delta = viewport.screen_to_world_relative([dx, dy]);

                    viewport.center = *camera_orig + delta;
                    viewport.upload();
                    self.window.request_redraw();
                }

                let point = viewport.screen_to_world_absolute(self.cursor);
                world.trigger(this, PointerEvent::Moved(point.floor()));

                self.window.request_redraw();
            }

            // Major Interaction //
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let viewport = world.single_fetch::<Viewport>().unwrap();
                let point = viewport.screen_to_world_absolute(self.cursor);
                world.trigger(this, PointerEvent::Pressed(point.floor()));

                self.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let viewport = world.single_fetch::<Viewport>().unwrap();
                let point = viewport.screen_to_world_absolute(self.cursor);
                world.trigger(this, PointerEvent::Released(point.floor()));

                self.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let viewport = world.single_fetch::<Viewport>().unwrap();
                let point = viewport.screen_to_world_absolute(self.cursor);
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
                let viewport = world.single_fetch::<Viewport>().unwrap();
                self.camera_cursor_start = self.cursor;
                self.camera_origin = Some(viewport.center);
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                self.camera_origin = None;
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_delta = match delta {
                    MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                    MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                };

                let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
                let cursor = viewport.screen_to_world_absolute(self.cursor);

                let follow = (viewport.center - cursor) * (-zoom_delta.into_f32()).exp2();
                viewport.center = cursor + follow;

                if let Some(camera_origin) = &mut self.camera_origin {
                    let follow = (*camera_origin - cursor) * (-zoom_delta.into_f32()).exp2();
                    *camera_origin = cursor + follow;
                }

                viewport.zoom += zoom_delta;

                viewport.upload();
                self.window.request_redraw();
            }

            // Render //
            WindowEvent::RedrawRequested => {
                let mut render = world.single_fetch_mut::<Render>().unwrap();
                render.redraw(world);
            }

            WindowEvent::Resized(size) => {
                let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

                viewport.size[0] = size.width.max(1);
                viewport.size[1] = size.height.max(1);

                viewport.upload();
                self.window.request_redraw();
            }

            WindowEvent::CloseRequested => {
                world.remove(this);
            }

            _ => (),
        }
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
