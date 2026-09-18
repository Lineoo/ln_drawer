use glam::{I64Vec2, UVec2};
use ln_world::{ElemRef, Element, Handle, HandleGeneric, World};

use crate::{
    layout::transform::TransformValue,
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl, RenderPhase,
        camera::{Camera, CameraBind, CameraDescriptor, CurrentCamera},
    },
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerScroll},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
        renderer::rrect::RRect,
    },
};

pub struct Container {
    pub rect: Rectangle,
    pub inner: Rectangle,
    pub inner_transform: TransformValue,
    pub visible: bool,
}

impl Container {
    pub fn init(&mut self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.insert(RRect {
            rect: self.rect,
            order: 0,
            color: theme.primary_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: self.visible,
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 0,
            enabled: self.visible,
        });

        // Contents of a container are laid out in the container's own local space and rendered
        // through a dedicated camera. Moving or scrolling the container only updates this
        // camera, so the children never have to be relaid out or re-uploaded to the GPU.
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();
        let lnwindow = world.single::<Lnwindow>().unwrap();
        let descriptor = {
            let current = world.single_fetch::<CurrentCamera>().unwrap();
            let parent = world.fetch(current.0).unwrap();
            CameraDescriptor {
                size: parent.size,
                center: I64Vec2::ZERO,
                zoom: parent.zoom,
            }
        };
        let camera = Camera::from_descriptor(descriptor, &render, &camera_bind.layout);
        drop(camera_bind);
        drop(render);

        let control = world.insert(RenderControl::phase_with_draw(
            handle,
            move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = world.single_fetch::<CurrentCamera>().unwrap();
                let camera = world.fetch(camera.0).unwrap();
                let panel_rect = world.fetch(handle).unwrap().rect;
                let window_size = lnwindow.window.surface_size();
                let left_up = lnwindow.screen_to_cursor(
                    camera.world_to_screen_absolute(I64Vec2::q32_from_i32(panel_rect.left_up())),
                );
                let right_down = lnwindow.screen_to_cursor(
                    camera.world_to_screen_absolute(I64Vec2::q32_from_i32(panel_rect.right_down())),
                );
                rpass.set_scissor_rect(
                    (left_up.x as u32).max(0),
                    (left_up.y as u32).max(0),
                    (right_down.x as u32).min(window_size.width) - (left_up.x as u32),
                    (right_down.y as u32).min(window_size.height) - (left_up.y as u32),
                );
                world.enter(handle, || {
                    let phase = &mut *world.single_fetch_mut::<RenderPhase>().unwrap();
                    phase.reorder();
                    phase.draw(world, rpass, extra);
                });
                rpass.set_scissor_rect(0, 0, window_size.width, window_size.height);
            },
        ));
        RenderControl::reorder(Some(isize::MAX), world, control);

        world.enter_queue(handle, move |world| {
            world.insert(RenderPhase::default());
            world.insert(ElemRef(lnwindow.untyped()));
            world.queue(move |world| {
                let camera = world.insert(camera);
                world.insert(CurrentCamera(camera));
            });
        });

        world.observer(collider, move |event: &PointerHit, world| {
            world.trigger(handle, event);
        });

        world.observer(collider, move |event: &PointerScroll, world| {
            world.trigger(handle, event);
        });

        let mut status = None;
        world.observer(handle, move |event: &PointerHit, world| {
            status = match (status, event.status) {
                (None, PointerHitStatus::Press | PointerHitStatus::Moving) => {
                    Some(event.pointer.screen)
                }
                (None, PointerHitStatus::Release) => None,
                (Some(_), PointerHitStatus::Press) => Some(event.pointer.screen),
                (Some(position), PointerHitStatus::Moving) => {
                    let current = world.single_fetch::<CurrentCamera>().unwrap();
                    let camera = world.fetch(current.0).unwrap();
                    let delta = camera.screen_to_world_relative(event.pointer.screen - position);
                    drop(camera);
                    move_camera(world, handle, delta);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    let current = world.single_fetch::<CurrentCamera>().unwrap();
                    let camera = world.fetch(current.0).unwrap();
                    let delta = camera.screen_to_world_relative(event.pointer.screen - position);
                    drop(camera);
                    move_camera(world, handle, delta);
                    None
                }
            }
        });

        world.observer(handle, move |event: &PointerScroll, world| {
            move_camera(world, handle, I64Vec2::q32_from_f64(event.delta));
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let old_origin = this.rect.origin;
            this.rect = rect;

            let extend = this.inner_transform.compute(rect).extend;
            let relayout = this.inner.extend != extend;
            this.inner = Rectangle::new_extend(0, 0, extend.x, extend.y);

            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = rect;
            drop(collider);

            world.queue_trigger(back, SetWidgetRectangle(rect));
            if relayout {
                world.queue_trigger(handle, WidgetRectangle(this.inner));
            }
            drop(this);
            move_camera(
                world,
                handle,
                I64Vec2::q32_from_i32(rect.origin - old_origin),
            );
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = enabled;
            collider.enabled = enabled;
            world.queue_trigger(back, SetWidgetVisible(enabled));
            world.queue_trigger(handle, WidgetVisible(enabled));
        });
    }
}

/// Move the container's internal camera by `delta` while keeping the contents inside the
/// container bounds.
pub(crate) fn move_camera(world: &World, handle: Handle<Container>, delta: I64Vec2) {
    world.enter(handle, || {
        let current = world.single_fetch::<CurrentCamera>().unwrap();
        let mut camera = world.fetch_mut(current.0).unwrap();
        let this = world.fetch(handle).unwrap();

        camera.center = -rect_contain(-camera.center + delta, this.inner.extend, this.rect);
    });
}

/// Keep a fixed-point content rectangle inside `viewport`.
///
/// Axes that overflow their viewport axis scroll normally. Axes that are too small to fill the
/// viewport cannot scroll, so the content is pinned to the top / left edge instead of drifting
/// freely (the conventional list behavior).
///
/// `origin` stays in q32 fixed point on purpose: rounding it to whole pixels on every move throws
/// away the sub-pixel remainder, so slow drags snap between pixels and the content drifts away
/// from the pointer. The shader already consumes the fractional camera offset, so keep it.
fn rect_contain(origin: I64Vec2, extend: UVec2, viewport: Rectangle) -> I64Vec2 {
    let x = match extend.x >= viewport.width() {
        true => {
            let min = i64::q32_from_i32(viewport.right() - extend.x as i32);
            let max = i64::q32_from_i32(viewport.left());
            origin.x.clamp(min, max)
        }
        false => i64::q32_from_i32(viewport.left()),
    };
    let y = match extend.y >= viewport.height() {
        true => {
            let min = i64::q32_from_i32(viewport.up() - extend.y as i32);
            let max = i64::q32_from_i32(viewport.down());
            origin.y.clamp(min, max)
        }
        false => i64::q32_from_i32(viewport.up() - extend.y as i32),
    };

    I64Vec2::new(x, y)
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
