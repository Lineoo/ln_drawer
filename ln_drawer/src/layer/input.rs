use std::time::{Duration, Instant};

use glam::{DVec2, UVec2};
use ln_world::{Element, Handle, World};
use winit::{
    event::{ElementState, PointerKind, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    layer::{
        brush::Draw,
        wrapper::{BrushMode, LayerWrapper},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::camera::{Camera, CameraUtils, UICamera},
    tools::{
        collider::ToolCollider,
        modifiers::ModifiersTool,
        pointer::{PointerHover, PointerHoverStatus},
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    widgets::{SetWidgetRectangle, SetWidgetVisible},
};

const DRAG_DISTANCE: f64 = 0.005;
const ERASE_TIMER: f64 = 0.4;

#[derive(Default)]
pub struct LayerInput {
    space: bool,
}

enum LayerInputState {
    None,
    Paint {
        start_position: DVec2,
        start_instant: Instant,
    },
    PaintNoErase,
    PaintErase,
    Grab {
        start_position: DVec2,
        start_pinch: Option<f64>,
    },
}

impl LayerInput {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider::fullscreen(-100));
        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHover, world| {
            if let PointerKind::Touch(_) = event.pointer.kind {
                return;
            }

            let ui_camera = world.single_fetch::<UICamera>().unwrap();
            world.enter(ui_camera.0, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
                let mut brush_preview = world.fetch_mut(wrapper.brush_preview).unwrap();
                brush_preview.desc.shadow_offset = event.pointer.tilt * 48.0;
                world.queue_trigger(
                    wrapper.brush_preview,
                    SetWidgetRectangle(Rectangle::new_half(
                        camera
                            .screen_to_world_absolute(event.pointer.screen)
                            .q32_round(),
                        UVec2::new(1, 1),
                    )),
                );

                match event.status {
                    PointerHoverStatus::Enter => {
                        world.queue_trigger(wrapper.brush_preview, SetWidgetVisible(true));
                    }
                    PointerHoverStatus::Moving => {}
                    PointerHoverStatus::Leave => {
                        world.queue_trigger(wrapper.brush_preview, SetWidgetVisible(false));
                    }
                }
            });
        });

        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            let WindowEvent::KeyboardInput { event, .. } = event else {
                return;
            };

            if event.repeat {
                return;
            }

            let modifier = world.single_fetch::<ModifiersTool>().unwrap();
            let ctrl = modifier.modifiers.state().control_key();
            let shift = modifier.modifiers.state().shift_key();
            let press = event.state == ElementState::Pressed;

            match KeyCode::from(event.physical_key) {
                KeyCode::KeyZ if press && ctrl => {
                    let mut wrapper = world.single_fetch_mut::<LayerWrapper>().unwrap();

                    if !shift {
                        wrapper.undo();
                    } else {
                        wrapper.redo();
                    }

                    let lnwindow = world.fetch(lnwindow).unwrap();
                    lnwindow.window.request_redraw();
                }
                KeyCode::Space => {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.space = press;
                }
                _ => (),
            }
        });

        let mut state = LayerInputState::None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let this = world.fetch(this).unwrap();
            let camera_utils = &mut *world.single_fetch_mut::<CameraUtils>().unwrap();
            let wrapper = &mut *world.single_fetch_mut::<LayerWrapper>().unwrap();
            let center = touch_center(event);
            let pinch = touch_pinch(event);

            let prev = std::mem::replace(&mut state, LayerInputState::None);
            state = match (prev, event.active.status) {
                // Grab
                (LayerInputState::None, MultiTouchStatus::Press)
                    if matches!(event.active.pointer, PointerKind::Touch(_)) || this.space =>
                {
                    camera_utils.camera_cursor_by_anchor_center(center);
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                    }

                    LayerInputState::Grab {
                        start_position: center,
                        start_pinch: pinch,
                    }
                }
                (LayerInputState::Grab { .. }, MultiTouchStatus::Press) => {
                    camera_utils.camera_cursor_by_anchor_center(center);
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                    }

                    LayerInputState::Grab {
                        start_position: center,
                        start_pinch: pinch,
                    }
                }
                (LayerInputState::Grab { .. }, MultiTouchStatus::Release) => {
                    if event.members.len() > 1 {
                        camera_utils.camera_cursor_by_anchor_center(center);
                        if let Some(distance) = pinch {
                            camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                        }

                        LayerInputState::Grab {
                            start_position: center,
                            start_pinch: pinch,
                        }
                    } else {
                        LayerInputState::None
                    }
                }
                (
                    LayerInputState::Grab {
                        start_position,
                        start_pinch,
                    },
                    MultiTouchStatus::Holding,
                ) => {
                    camera_utils.camera_cursor_by_camera_center(center);
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_camera_zoom_center(distance);
                    }
                    camera_utils.apply_to_camera(world);

                    LayerInputState::Grab {
                        start_position,
                        start_pinch,
                    }
                }

                // Paint
                (LayerInputState::None, MultiTouchStatus::Press) => LayerInputState::Paint {
                    start_position: event.active.screen,
                    start_instant: Instant::now(),
                },
                (LayerInputState::Paint { .. }, MultiTouchStatus::Press) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    draw_wrapper(wrapper, draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::PaintNoErase
                }
                (
                    LayerInputState::Paint {
                        start_position,
                        start_instant,
                    },
                    MultiTouchStatus::Holding,
                ) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    draw_wrapper(wrapper, draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    if event.active.screen.distance(start_position) > DRAG_DISTANCE {
                        LayerInputState::PaintNoErase
                    } else if start_instant.elapsed() > Duration::from_secs_f64(ERASE_TIMER) {
                        wrapper.brush.discard();
                        LayerInputState::PaintErase
                    } else {
                        LayerInputState::Paint {
                            start_position,
                            start_instant,
                        }
                    }
                }
                (
                    LayerInputState::PaintNoErase,
                    MultiTouchStatus::Press | MultiTouchStatus::Holding,
                ) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    draw_wrapper(wrapper, draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::PaintNoErase
                }
                (
                    LayerInputState::PaintErase,
                    MultiTouchStatus::Press | MultiTouchStatus::Holding,
                ) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    erase_wrapper(wrapper, draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::PaintErase
                }
                (
                    p @ (LayerInputState::Paint { .. }
                    | LayerInputState::PaintErase
                    | LayerInputState::PaintNoErase),
                    MultiTouchStatus::Release,
                ) => {
                    if event.members.len() > 1 {
                        p
                    } else {
                        wrapper.stock();
                        (wrapper.brush).submit(&mut wrapper.main, Some(&wrapper.thread_tx));
                        LayerInputState::None
                    }
                }

                // Edge cases
                (LayerInputState::None, MultiTouchStatus::Holding) => LayerInputState::None,
                (LayerInputState::None, MultiTouchStatus::Release) => LayerInputState::None,
            };
        });
    }
}

fn draw_wrapper(wrapper: &mut LayerWrapper, draw: Draw) {
    match &wrapper.brush_mode {
        BrushMode::Round => {
            (wrapper.brush).draw(&wrapper.main, &wrapper.round_brush, draw);
        }
        BrushMode::Blur => {
            (wrapper.brush).draw(&wrapper.main, &wrapper.blur_brush, draw);
        }
    };

    (wrapper.brush).request_stream(&wrapper.main, &wrapper.thread_tx);
}

fn erase_wrapper(wrapper: &mut LayerWrapper, draw: Draw) {
    (wrapper.brush).draw(&wrapper.main, &wrapper.temp_erase, draw);
    (wrapper.brush).request_stream(&wrapper.main, &wrapper.thread_tx);
}

fn touch_center(event: &MultiTouchGroup) -> DVec2 {
    let mut sum = DVec2::ZERO;
    let mut cnt = 0;
    for member in &event.members {
        if let MultiTouchStatus::Release = member.status {
            continue;
        }
        sum += member.screen;
        cnt += 1;
    }
    sum / cnt as f64
}

fn touch_pinch(event: &MultiTouchGroup) -> Option<f64> {
    if event.members.len() == 2 {
        let first = event.members.first().unwrap();
        let last = event.members.last().unwrap();

        Some((first.screen).distance(last.screen))
    } else {
        None
    }
}

impl Element for LayerInput {
    fn when_insert(&mut self, world: &ln_world::World, this: ln_world::Handle<Self>) {
        self.init(world, this);
    }
}
