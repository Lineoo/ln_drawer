use glam::{DVec2, IVec2};
use ln_world::{Element, Handle, World};
use winit::event::{
    ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::FI64Ext,
    render::camera::{Camera, CameraUtils, MainCamera},
    tools::collider::ToolCollider,
};

/// Mouse-specific operations like right-click and middle-click.
#[derive(Default)]
pub struct MouseTool;

/// Right-click events.
#[derive(Clone, Copy)]
#[expect(unused)]
pub struct MouseMenu(pub IVec2);

impl Element for MouseTool {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();

        let mut middle = false;
        let mut prev = DVec2::ZERO;
        world.observer(lnwindow, move |event: &WindowEvent, world| match event {
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
                    camera.screen_to_world_absolute(screen).q32_floor()
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
                let main = world.single_fetch::<MainCamera>().unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);
                middle = true;

                world.enter(main.0, || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();
                    camera_utils.anchor_cursor(DVec2::ZERO);
                    camera_utils.camera_cursor_by_anchor_center(cursor);
                });
            }

            WindowEvent::PointerMoved {
                position,
                source: PointerSource::Mouse,
                ..
            } => {
                let main = world.single_fetch::<MainCamera>().unwrap();
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);
                prev = cursor;

                world.enter(main.0, || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();
                    if middle {
                        camera_utils.camera_cursor_by_camera_center(cursor);
                        camera_utils.apply_to_camera(world);
                    }
                });
            }

            WindowEvent::PointerButton {
                state: ElementState::Released,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                middle = false;
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let main = world.single_fetch::<MainCamera>().unwrap();
                world.enter(main.0, || {
                    let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                    let zoom_delta = match delta {
                        MouseScrollDelta::LineDelta(_rows, lines) => *lines as f64 / 4.0,
                        MouseScrollDelta::PixelDelta(delta) => delta.y / 16.0,
                    };

                    camera_utils.anchor_cursor(DVec2::ZERO);
                    camera_utils.camera_cursor_by_anchor_center(prev);
                    camera_utils.anchor_distance(1.0);
                    camera_utils.camera_distance_by_camera_zoom_center(1.0 + zoom_delta);
                    camera_utils.camera_distance_by_anchor_zoom_cursor(1.0);
                    camera_utils.apply_to_camera(world);
                });
            }

            _ => {}
        });
    }
}
