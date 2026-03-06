use winit::event::{
    ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position, PositionFract},
    render::viewport::Viewport,
    tools::{pointer::PointerCollider, viewport::ViewportUtils},
    world::{Element, Handle, World},
};

/// Mouse-specific operations like right-click and middle-click.
#[derive(Default)]
pub struct MouseTool;

/// Right-click events.
// TODO specific Menu collider
#[derive(Clone, Copy)]
pub struct PointerMenu(pub Position);

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
                let viewport = world.single_fetch::<Viewport>().unwrap();

                let position = lnwindow.cursor_to_screen(*position);
                let position = viewport.screen_to_world_absolute(position);

                let target = PointerCollider::intersect(world, position.floor())
                    .first()
                    .copied();

                if let Some(target) = target {
                    world.trigger(target, &PointerMenu(position.floor()));
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
                let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();

                let cursor = lnwindow.cursor_to_screen(*position);
                viewport_utils.cursor(world, cursor);
                viewport_utils.anchor_on_screen(world, cursor);
                viewport_utils.locked(true);
            }

            WindowEvent::PointerMoved {
                position,
                source: PointerSource::Mouse,
                ..
            } => {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();

                let cursor = lnwindow.cursor_to_screen(*position);
                viewport_utils.cursor(world, cursor);
            }

            WindowEvent::PointerButton {
                position,
                state: ElementState::Released,
                button: ButtonSource::Mouse(MouseButton::Middle),
                ..
            } => {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();

                let cursor = lnwindow.cursor_to_screen(*position);
                viewport_utils.cursor(world, cursor);
                viewport_utils.locked(false);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let mut viewport_utils = world.single_fetch_mut::<ViewportUtils>().unwrap();

                let zoom_delta = match delta {
                    MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                    MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                };

                viewport_utils.zoom_delta(world, zoom_delta);
            }

            _ => {}
        });
    }
}
