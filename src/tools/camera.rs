use winit::event::{
    ButtonSource, ElementState, FingerId, MouseButton, MouseScrollDelta, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, PositionFract},
    render::viewport::Viewport,
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct CameraTool {
    cursor: [f64; 2],
    start: Option<Start>,
}

enum Start {
    Cursor {
        cursor: [f64; 2],
        center: PositionFract,
    },
    Touch {
        position: [f64; 2],
        touch_id: FingerId,
        center: PositionFract,
    },
}

impl Element for CameraTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            world.single::<Lnwindow>().unwrap(),
            move |event: &WindowEvent, world, lnwindow| match event {
                WindowEvent::PointerMoved {
                    position,
                    source: PointerSource::Mouse,
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    let position = lnwindow.cursor_to_screen(*position);

                    this.cursor = position;
                    if let Some(Start::Cursor { cursor, center }) = &mut this.start {
                        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
                        let delta = viewport.screen_to_world_relative([
                            cursor[0] - position[0],
                            cursor[1] - position[1],
                        ]);

                        viewport.center = *center + delta;
                    }
                }

                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    button: ButtonSource::Mouse(MouseButton::Middle),
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    if let None = this.start {
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        this.start = Some(Start::Cursor {
                            cursor: this.cursor,
                            center: viewport.center,
                        });
                    }
                }

                WindowEvent::PointerButton {
                    state: ElementState::Released,
                    button: ButtonSource::Mouse(MouseButton::Middle),
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    if let Some(Start::Cursor { .. }) = this.start {
                        this.start = None;
                    }
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    let zoom_delta = match delta {
                        MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                        MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                    };

                    let this = &mut *world.fetch_mut(this).unwrap();
                    let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

                    let world_cursor = viewport.screen_to_world_absolute(this.cursor);

                    let follow = (viewport.center - world_cursor) * (-zoom_delta).exp2();
                    viewport.center = world_cursor + follow;

                    if let Some(Start::Cursor { center, .. }) = &mut this.start {
                        let follow = (*center - world_cursor) * (-zoom_delta).exp2();
                        *center = world_cursor + follow;
                    }

                    viewport.zoom += zoom_delta;
                }

                WindowEvent::PointerMoved {
                    position,
                    source: PointerSource::Touch { finger_id, .. },
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    if let Some(Start::Touch {
                        position: start,
                        touch_id,
                        center,
                    }) = &mut this.start
                        && touch_id == finger_id
                    {
                        let lnwindow = world.fetch(lnwindow).unwrap();
                        let screen = lnwindow.cursor_to_screen(*position);
                        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

                        let delta = viewport
                            .screen_to_world_relative([start[0] - screen[0], start[1] - screen[1]]);

                        viewport.center = *center + delta;
                    }
                }

                WindowEvent::PointerButton {
                    position,
                    state: ElementState::Pressed,
                    button: ButtonSource::Touch { finger_id, .. },
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    if let None = this.start {
                        let lnwindow = world.fetch(lnwindow).unwrap();
                        let position = lnwindow.cursor_to_screen(*position);
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        this.start = Some(Start::Touch {
                            position,
                            touch_id: *finger_id,
                            center: viewport.center,
                        });
                    }
                }

                WindowEvent::PointerButton {
                    state: ElementState::Released,
                    button: ButtonSource::Touch { finger_id, .. },
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    if let Some(Start::Touch { touch_id, .. }) = this.start
                        && touch_id == *finger_id
                    {
                        this.start = None;
                    }
                }

                _ => {}
            },
        );
    }
}
