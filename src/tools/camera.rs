use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, PositionFract},
    render::viewport::Viewport,
    world::{Element, Handle, World},
};

#[derive(Debug, Default)]
pub struct CameraTool {
    cursor: [f64; 2],
    start: Option<Start>,
}

#[derive(Debug)]
struct Start {
    cursor: [f64; 2],
    center: PositionFract,
}

impl Element for CameraTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            world.single::<Lnwindow>().unwrap(),
            move |event: &WindowEvent, world, lnwindow| match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let mut this = world.fetch_mut(this).unwrap();

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    let position = lnwindow.cursor_to_screen(*position);

                    this.cursor = position;
                    if let Some(start) = &mut this.start {
                        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
                        let delta = viewport.screen_to_world_relative([
                            start.cursor[0] - position[0],
                            start.cursor[1] - position[1],
                        ]);

                        viewport.center = start.center + delta;
                        viewport.upload();
                    }

                    lnwindow.request_redraw();
                }

                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Middle,
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    let viewport = world.single_fetch::<Viewport>().unwrap();
                    this.start = Some(Start {
                        cursor: this.cursor,
                        center: viewport.center,
                    });

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    lnwindow.request_redraw();
                }

                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Middle,
                    ..
                } => {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.start = None;

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    lnwindow.request_redraw();
                }

                WindowEvent::MouseWheel { delta, .. } => {
                    let zoom_delta = match delta {
                        MouseScrollDelta::LineDelta(_rows, lines) => Fract::from_f32(*lines),
                        MouseScrollDelta::PixelDelta(delta) => Fract::from_f64(delta.y / 16.0),
                    };

                    let this = &mut *world.fetch_mut(this).unwrap();
                    let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

                    let cursor = viewport.screen_to_world_absolute(this.cursor);

                    let follow = (viewport.center - cursor) * (-zoom_delta).exp2();
                    viewport.center = cursor + follow;

                    if let Some(start) = &mut this.start {
                        let follow = (start.center - cursor) * (-zoom_delta).exp2();
                        start.center = cursor + follow;
                    }

                    viewport.zoom += zoom_delta;
                    viewport.upload();

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    lnwindow.request_redraw();
                }

                _ => {}
            },
        );
    }
}
