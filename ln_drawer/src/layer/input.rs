use std::time::{Duration, Instant};

use glam::{DVec2, UVec2};
use ln_world::{Element, Handle, World};
use palette::IntoColor;
use winit::{
    cursor::{Cursor, CursorIcon},
    event::{ElementState, PointerKind, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    layer::{
        brush::Draw,
        wrapper::{BrushConfigurationChanged, BrushMode, LayerWrapper},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::camera::{CameraUtils, UICamera},
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
    pub touch_draw: bool,
    pub space: bool,
    pub ctrl: bool,
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
    Scale {
        start_position: DVec2,
    },
    PickColor,
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
                let camera = world.fetch(ui_camera.0).unwrap();
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

            let mut this = world.fetch_mut(this).unwrap();
            let lnwindow = world.fetch(lnwindow).unwrap();
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

                    lnwindow.window.request_redraw();
                }
                KeyCode::Space => {
                    this.space = press;
                }
                KeyCode::ControlLeft => {
                    this.ctrl = press;
                }
                _ => (),
            }

            update_icon(&this, &LayerInputState::None, &lnwindow);
        });

        let mut state = LayerInputState::None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let this = world.fetch(this).unwrap();
            let lnwindow = world.fetch(lnwindow).unwrap();
            let camera_utils = &mut *world.single_fetch_mut::<CameraUtils>().unwrap();
            let wrapper = &mut *world.single_fetch_mut::<LayerWrapper>().unwrap();
            let center = touch_center(event);
            let pinch = touch_pinch(event);

            let prev = std::mem::replace(&mut state, LayerInputState::None);
            state = match (prev, event.active.status) {
                (LayerInputState::None, MultiTouchStatus::Press)
                    if (!this.touch_draw
                        && matches!(event.active.pointer, PointerKind::Touch(_)))
                        || this.space =>
                {
                    camera_utils.camera_cursor_by_anchor_center(center);
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                    }

                    if this.ctrl {
                        camera_utils.camera_distance_by_anchor_zoom_cursor(1.0);
                        LayerInputState::Scale {
                            start_position: center,
                        }
                    } else {
                        LayerInputState::Grab {
                            start_position: center,
                            start_pinch: pinch,
                        }
                    }
                }
                (LayerInputState::None, MultiTouchStatus::Press) if this.ctrl => {
                    pick_color(event, world, wrapper);
                    LayerInputState::PickColor
                }

                // Grab
                (
                    LayerInputState::Grab { .. },
                    MultiTouchStatus::Press | MultiTouchStatus::Release,
                ) => {
                    if event.members.len() > 0 {
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

                    if this.ctrl {
                        camera_utils.camera_cursor_by_anchor_center(center);
                        camera_utils.camera_distance_by_anchor_zoom_cursor(1.0);
                        LayerInputState::Scale {
                            start_position: center,
                        }
                    } else {
                        LayerInputState::Grab {
                            start_position,
                            start_pinch,
                        }
                    }
                }

                // Scale
                (
                    LayerInputState::Scale { .. },
                    MultiTouchStatus::Press | MultiTouchStatus::Release,
                ) => {
                    if event.members.len() > 0 {
                        camera_utils.camera_cursor_by_anchor_center(center);
                        if let Some(distance) = pinch {
                            camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                        } else {
                            camera_utils.camera_distance_by_anchor_zoom_cursor(1.0);
                        }

                        LayerInputState::Scale {
                            start_position: center,
                        }
                    } else {
                        LayerInputState::None
                    }
                }
                (LayerInputState::Scale { start_position }, MultiTouchStatus::Holding) => {
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_camera_zoom_center(distance);
                    } else {
                        camera_utils.camera_distance_by_camera_zoom_center(
                            (center - start_position).element_sum().exp2(),
                        );
                    }
                    camera_utils.apply_to_camera(world);

                    if this.ctrl {
                        LayerInputState::Scale { start_position }
                    } else {
                        camera_utils.camera_cursor_by_anchor_center(center);
                        if let Some(distance) = pinch {
                            camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                        }
                        LayerInputState::Grab {
                            start_position: center,
                            start_pinch: pinch,
                        }
                    }
                }

                // Paint
                (LayerInputState::None, MultiTouchStatus::Press) => LayerInputState::Paint {
                    start_position: event.active.screen,
                    start_instant: Instant::now(),
                },
                (
                    LayerInputState::Paint { .. }
                    | LayerInputState::PaintErase
                    | LayerInputState::PaintNoErase,
                    MultiTouchStatus::Press,
                ) => {
                    wrapper.brush.discard();

                    camera_utils.camera_cursor_by_anchor_center(center);
                    if let Some(distance) = pinch {
                        camera_utils.camera_distance_by_anchor_zoom_cursor(distance);
                    }

                    LayerInputState::Grab {
                        start_position: center,
                        start_pinch: pinch,
                    }
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
                (LayerInputState::PaintNoErase, MultiTouchStatus::Holding) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    draw_wrapper(wrapper, draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::PaintNoErase
                }
                (LayerInputState::PaintErase, MultiTouchStatus::Holding) => {
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

                (
                    LayerInputState::PickColor,
                    MultiTouchStatus::Press | MultiTouchStatus::Holding,
                ) => {
                    pick_color(event, world, wrapper);
                    LayerInputState::PickColor
                }
                (LayerInputState::PickColor, MultiTouchStatus::Release) => LayerInputState::None,

                // Edge cases
                (LayerInputState::None, MultiTouchStatus::Holding) => LayerInputState::None,
                (LayerInputState::None, MultiTouchStatus::Release) => LayerInputState::None,
            };

            update_icon(&this, &state, &lnwindow);
        });
    }
}

fn pick_color(event: &MultiTouchGroup, world: &World, wrapper: &mut LayerWrapper) {
    let cmd = world.commander();
    wrapper.brush.layer.pick_color(
        &wrapper.main,
        event.active.position.q32_round(),
        move |color| {
            cmd.queue(move |world| {
                let mut wrapper = world.single_fetch_mut::<LayerWrapper>().unwrap();
                wrapper.round_brush.color = color.into_color();
                wrapper.tint_brush.color = color.into_color();
                world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
            });
        },
    );
}

fn update_icon(this: &LayerInput, state: &LayerInputState, lnwindow: &Lnwindow) {
    match (this.space, this.ctrl, state) {
        (true, true, LayerInputState::None) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::ZoomIn));
        }
        (true, false, LayerInputState::None) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::Grab));
        }
        (false, true, LayerInputState::None) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        }
        (_, _, LayerInputState::PickColor) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        }
        (_, _, LayerInputState::Grab { .. }) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Grabbing));
        }
        (_, _, LayerInputState::Scale { .. }) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::ZoomIn));
        }
        _ => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Default));
        }
    }
}

fn draw_wrapper(wrapper: &mut LayerWrapper, draw: Draw) {
    match &wrapper.brush_mode {
        BrushMode::Round => (wrapper.brush).draw(&wrapper.main, &wrapper.round_brush, draw),
        BrushMode::Blur => (wrapper.brush).draw(&wrapper.main, &wrapper.blur_brush, draw),
        BrushMode::Tint => (wrapper.brush).draw(&wrapper.main, &wrapper.tint_brush, draw),
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
