use std::time::{Duration, Instant};

use glam::{DVec2, UVec2};
use ln_world::{Element, Handle, World};
use winit::{
    cursor::{Cursor, CursorIcon},
    event::{ElementState, PointerKind, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    layer::{
        brush::{Draw, param::BrushParamKey},
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::camera::{CameraUtils, MainCamera, UICamera},
    tools::{
        collider::ToolCollider,
        pointer::{PointerHover, PointerHoverStatus, PointerScroll},
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
    pub shift: bool,
    pub pick: bool,
    pub hold_pick: bool,
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
        let main_camera = world.single_fetch::<MainCamera>().unwrap().0;
        let collider = world.insert(ToolCollider::fullscreen(-100, main_camera));
        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHover, world| {
            if let PointerKind::Touch(_) = event.pointer.kind {
                return;
            }

            let ui_camera = world.single_fetch::<UICamera>().unwrap();
            world.enter(ui_camera.0, || {
                let camera = world.fetch(ui_camera.0).unwrap();
                let wrapper = world.single_fetch::<LayerPage>().unwrap();
                let brush_rect = Rectangle::new_half(
                    camera.dst_to_src(event.pointer.screen).q32_round(),
                    UVec2::new(1, 1),
                );
                let shadow_rect = brush_rect + (event.pointer.tilt * 48.0).as_ivec2();
                world.queue_trigger(wrapper.brush_preview, SetWidgetRectangle(brush_rect));
                world.queue_trigger(
                    wrapper.brush_preview_shadow,
                    SetWidgetRectangle(shadow_rect),
                );

                match event.status {
                    PointerHoverStatus::Enter => {
                        world.queue_trigger(wrapper.brush_preview, SetWidgetVisible(true));
                        world.queue_trigger(wrapper.brush_preview_shadow, SetWidgetVisible(true));
                    }
                    PointerHoverStatus::Moving => {}
                    PointerHoverStatus::Leave => {
                        world.queue_trigger(wrapper.brush_preview, SetWidgetVisible(false));
                        world.queue_trigger(wrapper.brush_preview_shadow, SetWidgetVisible(false));
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
            let ctrl = this.ctrl;
            let shift = this.shift;
            let press = event.state == ElementState::Pressed;

            match KeyCode::from(event.physical_key) {
                KeyCode::KeyZ if press && ctrl => {
                    let mut page = world.single_fetch_mut::<LayerPage>().unwrap();

                    if !shift {
                        page.undo();
                    } else {
                        page.redo();
                    }

                    lnwindow.window.request_redraw();
                }
                KeyCode::KeyE if press => {
                    let mut page = world.single_fetch_mut::<LayerPage>().unwrap();
                    let active = page.active_mut();
                    let is_erase = active.toggle(BrushParamKey::Erase).unwrap_or_default();
                    active.set_toggle(BrushParamKey::Erase, !is_erase);
                    world.queue_trigger(page.handle(), BrushConfigurationChanged);
                }
                KeyCode::Space => {
                    this.space = press;
                }
                KeyCode::ControlLeft => {
                    this.ctrl = press;
                }
                KeyCode::ShiftLeft => {
                    this.shift = press;
                }
                _ => (),
            }

            update_icon(&this, &LayerInputState::None, &lnwindow);
        });

        world.observer(collider, move |event: &PointerScroll, world| {
            let main = world.single_fetch::<MainCamera>().unwrap();
            world.enter(main.0, || {
                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                let zoom_delta = -event.delta.y;
                camera_utils.anchor_cursor(DVec2::ZERO);
                camera_utils.camera_cursor_by_anchor_center(event.pointer.screen);
                camera_utils.anchor_distance(200.0);
                if zoom_delta > 0.0 {
                    camera_utils.camera_distance_by_anchor_zoom_cursor(200.0);
                    camera_utils.camera_distance_by_camera_zoom_center(200.0 + zoom_delta);
                } else {
                    camera_utils.camera_distance_by_anchor_zoom_cursor(200.0 - zoom_delta);
                    camera_utils.camera_distance_by_camera_zoom_center(200.0);
                }
                camera_utils.apply_to_camera(world, main.0);
            });
        });

        let mut state = LayerInputState::None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let main_camera = world.single_fetch::<MainCamera>().unwrap().0;
            let mut this = world.fetch_mut(this).unwrap();
            let lnwindow = world.fetch(lnwindow).unwrap();
            let camera_utils = &mut *world.single_fetch_mut::<CameraUtils>().unwrap();
            let page = &mut *world.single_fetch_mut::<LayerPage>().unwrap();
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
                (LayerInputState::None, MultiTouchStatus::Press) if this.ctrl || this.pick => {
                    page.pick_color(event.active.position, world);
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
                    camera_utils.apply_to_camera(world, main_camera);

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
                    camera_utils.apply_to_camera(world, main_camera);

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
                (LayerInputState::None, MultiTouchStatus::Press) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    page.draw_active(draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::Paint {
                        start_position: event.active.screen,
                        start_instant: Instant::now(),
                    }
                }
                (
                    LayerInputState::Paint { .. }
                    | LayerInputState::PaintErase
                    | LayerInputState::PaintNoErase,
                    MultiTouchStatus::Press,
                ) => {
                    page.discard();

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

                    page.draw_active(draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    if event.active.screen.distance(start_position) > DRAG_DISTANCE {
                        LayerInputState::PaintNoErase
                    } else if start_instant.elapsed() > Duration::from_secs_f64(ERASE_TIMER) {
                        page.discard();
                        if this.hold_pick {
                            LayerInputState::PickColor
                        } else {
                            LayerInputState::PaintErase
                        }
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

                    page.draw_active(draw);

                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    lnwindow.window.request_redraw();

                    LayerInputState::PaintNoErase
                }
                (LayerInputState::PaintErase, MultiTouchStatus::Holding) => {
                    let draw = Draw {
                        position: event.active.position,
                        force: event.active.data.force.unwrap_or(1.0),
                    };

                    page.draw_erase(draw);

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
                        page.submit();
                        LayerInputState::None
                    }
                }

                (
                    LayerInputState::PickColor,
                    MultiTouchStatus::Press | MultiTouchStatus::Holding,
                ) => {
                    page.pick_color(event.active.position, world);
                    LayerInputState::PickColor
                }
                (LayerInputState::PickColor, MultiTouchStatus::Release) => {
                    page.pick_color(event.active.position, world);
                    this.pick = false;
                    world.queue_trigger(
                        world.single::<LayerPage>().unwrap(),
                        BrushConfigurationChanged,
                    );
                    LayerInputState::None
                }

                // Edge cases
                (LayerInputState::None, MultiTouchStatus::Holding) => LayerInputState::None,
                (LayerInputState::None, MultiTouchStatus::Release) => LayerInputState::None,
            };

            update_icon(&this, &state, &lnwindow);
        });
    }
}

fn update_icon(this: &LayerInput, state: &LayerInputState, lnwindow: &Lnwindow) {
    match (this.space, this.ctrl, this.pick, state) {
        (true, true, _, LayerInputState::None) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::ZoomIn));
        }
        (true, false, _, LayerInputState::None) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::Grab));
        }
        (false, true, _, LayerInputState::None) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        }
        (false, _, true, LayerInputState::None) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        }
        (.., LayerInputState::PickColor) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Crosshair));
        }
        (.., LayerInputState::Grab { .. }) => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Grabbing));
        }
        (.., LayerInputState::Scale { .. }) => {
            lnwindow.window.set_cursor(Cursor::Icon(CursorIcon::ZoomIn));
        }
        _ => {
            lnwindow
                .window
                .set_cursor(Cursor::Icon(CursorIcon::Default));
        }
    }
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
