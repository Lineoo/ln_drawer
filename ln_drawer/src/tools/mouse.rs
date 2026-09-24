use glam::{DVec2, IVec2};
use ln_world::{Element, Handle, World};
use winit::event::{ButtonSource, ElementState, MouseButton, PointerSource, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::FI64Ext,
    render::camera::{MainCamera, MainCameraUtils},
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

                let Some(hit) = ToolCollider::intersect(world, screen) else {
                    return;
                };

                world.queue_trigger(hit.collider, MouseMenu(hit.position.q32_floor()));
            }

            // middle-click //
            WindowEvent::PointerButton {
                position,
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let utils = world.single_fetch::<MainCameraUtils>().unwrap().0;
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);
                middle = true;

                let mut camera_utils = world.fetch_mut(utils).unwrap();
                camera_utils.anchor_cursor(DVec2::ZERO);
                camera_utils.camera_cursor_by_anchor_center(cursor);
            }

            WindowEvent::PointerMoved {
                position,
                source: PointerSource::Mouse,
                ..
            } => {
                let main = world.single_fetch::<MainCamera>().unwrap().0;
                let utils = world.single_fetch::<MainCameraUtils>().unwrap().0;
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let cursor = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let mut camera_utils = world.fetch_mut(utils).unwrap();
                if middle {
                    camera_utils.camera_cursor_by_camera_center(cursor);
                    camera_utils.apply_to_camera(world, main);
                }
            }

            WindowEvent::PointerButton {
                state: ElementState::Released,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                middle = false;
            }

            _ => {}
        });
    }
}
