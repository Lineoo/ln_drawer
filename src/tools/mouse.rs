use winit::event::{ButtonSource, ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position},
    render::camera::{Camera, CameraUtils},
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
                let camera = world.single_fetch::<Camera>().unwrap();

                let position = lnwindow.cursor_to_screen(*position);
                let position = camera.screen_to_world_absolute(position);

                let target = ToolCollider::intersect(world, position.floor())
                    .first()
                    .copied();

                if let Some(target) = target {
                    world.trigger(target, &MouseMenu(position.floor()));
                }
            }

            // middle-click //
            WindowEvent::PointerButton {
                position,
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                let cursor = lnwindow.cursor_to_screen(*position);
                camera_utils.cursor(world, cursor);
                camera_utils.anchor_on_screen(world, cursor);
                camera_utils.locked(true);
            }

            WindowEvent::PointerButton {
                position,
                state: ElementState::Released,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                let cursor = lnwindow.cursor_to_screen(*position);
                camera_utils.cursor(world, cursor);
                camera_utils.locked(false);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                let zoom_delta = match delta {
                    MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                    MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                };

                camera_utils.zoom_delta(world, zoom_delta);
            }

            _ => {}
        });
    }
}
