use winit::event::{
    ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position},
    render::camera::{Camera, CameraUtils, CameraVisits},
    tools::collider::ToolCollider,
    world::{Element, Handle, World},
};

/// Mouse-specific operations like right-click and middle-click.
#[derive(Default)]
pub struct MouseTool;

/// Right-click events.
#[derive(Clone, Copy)]
pub struct MouseMenu(pub Position);

impl Element for MouseTool {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();

        world.observer(lnwindow, |event: &WindowEvent, world| match event {
            // right-click //
            WindowEvent::PointerButton {
                position,
                button: ButtonSource::Mouse(MouseButton::Right),
                state: ElementState::Pressed,
                ..
            } => {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let Some(&(target, view)) = ToolCollider::intersect(world, screen).first() else {
                    return;
                };

                let position = world.enter(view, || {
                    let camera = world.single_fetch::<Camera>().unwrap();
                    camera.screen_to_world_absolute(screen).floor()
                });

                world.queue_trigger(target, MouseMenu(position));
            }

            // middle-click //
            WindowEvent::PointerButton {
                position,
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let camera_visits = world.single_fetch::<CameraVisits>().unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);

                world.enter(camera_visits.views[0], || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();
                    camera_utils.cursor(world, cursor);
                    camera_utils.anchor_on_screen(world, cursor);
                    camera_utils.locked(true);
                });
            }

            WindowEvent::PointerMoved {
                position,
                source: PointerSource::Mouse,
                ..
            } => {
                let camera_visits = world.single_fetch::<CameraVisits>().unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);

                world.enter(camera_visits.views[0], || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();
                    camera_utils.cursor(world, cursor);
                });
            }

            WindowEvent::PointerButton {
                position,
                state: ElementState::Released,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let camera_visits = world.single_fetch::<CameraVisits>().unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);

                world.enter(camera_visits.views[0], || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                    camera_utils.cursor(world, cursor);
                    camera_utils.locked(false);
                });
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let camera_visits = world.single_fetch::<CameraVisits>().unwrap();
                world.enter(camera_visits.views[0], || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                    let zoom_delta = match delta {
                        MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                        MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                    };

                    camera_utils.zoom_delta(world, zoom_delta);
                });
            }

            _ => {}
        });
    }
}
