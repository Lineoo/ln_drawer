use glam::{I64Vec2, UVec2};
use ln_world::{ElemRef, Element, Handle, HandleGeneric, World};
use winit::dpi::PhysicalSize;

use crate::{
    layout::transform::TransformValue,
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl, ScissorRect,
        camera::{Camera, CameraBind, CameraDescriptor, CameraUpdated, CurrentCamera},
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

/// Per-container state that survives across the container's own lifetime.
///
/// `parent` is the camera of the view the container was inserted into. `scroll` is the position
/// of the container's local origin in that parent's coordinate space; the container camera is
/// always `parent.center - scroll`, so nested containers compose correctly.
struct ContainerState {
    parent: Handle<Camera>,
    scroll: I64Vec2,
}

impl Element for ContainerState {}

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
        let parent_camera = world.single_fetch::<CurrentCamera>().unwrap().0;
        let descriptor = {
            let parent = world.fetch(parent_camera).unwrap();
            CameraDescriptor {
                src_size: parent.src_size,
                src_center: parent.src_center,
                src_zoom: parent.src_zoom,
                dst_size: UVec2::splat(2),
                dst_center: I64Vec2::ZERO,
                dst_zoom: 0,
            }
        };
        let camera = Camera::from_descriptor(descriptor, &render, &camera_bind.layout);
        drop(camera_bind);
        drop(render);

        let state = world.insert(ContainerState {
            parent: parent_camera,
            scroll: I64Vec2::ZERO,
        });
        world.dependency(state, handle);

        world.observer(parent_camera, move |&CameraUpdated, world| {
            let Ok(state) = world.fetch(state) else {
                return;
            };
            let parent_center = match world.fetch(state.parent) {
                Ok(parent) => parent.src_center,
                Err(_) => return,
            };
            let scroll = state.scroll;
            drop(state);
            set_camera_center(world, handle, compose_center(parent_center, scroll));
        });

        world.enter_queue(handle, move |world| {
            world.insert(ElemRef(lnwindow.untyped()));
            world.queue(move |world| {
                let camera = world.insert(camera);
                world.insert(CurrentCamera(camera));
            });
        });

        let control = world.insert(RenderControl::phase_with_draw(
            handle,
            move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = extra.camera;
                let panel_rect = world.fetch(handle).unwrap().rect;
                let window_size = lnwindow.window.surface_size();

                let scissor = extra.scissor;
                let enclosing = scissor.get();
                let own = scissor_rect(&lnwindow, &camera, panel_rect, window_size);

                // A container clips to its own rect, intersected with whatever clip is already
                // active so nested containers cannot leak outside their parent.
                let clipped = match enclosing {
                    Some(prev) => prev.intersect(own).unwrap_or(ScissorRect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    }),
                    None => own,
                };
                scissor.set(Some(clipped));

                if clipped.width > 0 && clipped.height > 0 {
                    rpass.set_scissor_rect(clipped.x, clipped.y, clipped.width, clipped.height);
                    world.enter(handle, || {
                        let curr = world.single_fetch::<CurrentCamera>().unwrap();
                        let camera = &mut *world.fetch_mut(curr.0).unwrap();
                        camera.reorder();
                        camera.draw(world, rpass, extra);
                    });
                }

                scissor.set(enclosing);
                let restore = enclosing.unwrap_or(ScissorRect {
                    x: 0,
                    y: 0,
                    width: window_size.width,
                    height: window_size.height,
                });
                rpass.set_scissor_rect(restore.x, restore.y, restore.width, restore.height);
            },
        ));
        RenderControl::reorder(Some(isize::MAX), world, control);

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
                    let delta = camera.dst_to_src_relative(event.pointer.screen - position);
                    drop(camera);
                    move_camera(world, handle, state, delta);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    let current = world.single_fetch::<CurrentCamera>().unwrap();
                    let camera = world.fetch(current.0).unwrap();
                    let delta = camera.dst_to_src_relative(event.pointer.screen - position);
                    drop(camera);
                    move_camera(world, handle, state, delta);
                    None
                }
            }
        });

        world.observer(handle, move |event: &PointerScroll, world| {
            move_camera(world, handle, state, I64Vec2::q32_from_f64(event.delta));
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
                state,
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
///
/// `delta` is in the container's parent coordinate space. The scroll offset is converted into an
/// absolute camera center through the parent camera so nested containers compose.
fn move_camera(
    world: &World,
    handle: Handle<Container>,
    state: Handle<ContainerState>,
    delta: I64Vec2,
) {
    let parent_center = {
        let state = world.fetch(state).unwrap();
        world.fetch(state.parent).unwrap().src_center
    };

    let mut state = world.fetch_mut(state).unwrap();
    let this = world.fetch(handle).unwrap();
    state.scroll = rect_contain(state.scroll + delta, this.inner.extend, this.rect);
    let center = compose_center(parent_center, state.scroll);
    drop(this);
    drop(state);

    set_camera_center(world, handle, center);
}

/// Absolute camera center for a container whose parent camera is centered at `parent_center` and
/// whose content origin sits at `scroll` in the parent's coordinate space.
fn compose_center(parent_center: I64Vec2, scroll: I64Vec2) -> I64Vec2 {
    parent_center - scroll
}

/// Point the container's own camera at `center` (absolute, in the parent camera's space).
fn set_camera_center(world: &World, handle: Handle<Container>, center: I64Vec2) {
    world.enter(handle, || {
        let Ok(current) = world.single_fetch::<CurrentCamera>() else {
            return;
        };
        let Ok(mut camera) = world.fetch_mut(current.0) else {
            return;
        };
        if camera.src_center != center {
            camera.src_center = center;
        }
    });
}

/// Convert a rectangle in the parent camera's space into a pixel scissor clamped to the window.
fn scissor_rect(
    lnwindow: &Lnwindow,
    camera: &Camera,
    rect: Rectangle,
    window_size: PhysicalSize<u32>,
) -> ScissorRect {
    let left_up =
        lnwindow.screen_to_cursor(camera.src_to_dst(I64Vec2::q32_from_i32(rect.left_up())));
    let right_down =
        lnwindow.screen_to_cursor(camera.src_to_dst(I64Vec2::q32_from_i32(rect.right_down())));

    let x = (left_up.x.max(0.0) as u32).min(window_size.width);
    let y = (left_up.y.max(0.0) as u32).min(window_size.height);
    let right = (right_down.x.max(0.0) as u32).min(window_size.width);
    let down = (right_down.y.max(0.0) as u32).min(window_size.height);

    ScissorRect {
        x,
        y,
        width: right.saturating_sub(x),
        height: down.saturating_sub(y),
    }
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

#[cfg(test)]
mod tests {
    use glam::IVec2;

    use super::*;

    fn q(x: i32, y: i32) -> I64Vec2 {
        I64Vec2::q32_from_i32(IVec2::new(x, y))
    }

    fn rect(left: i32, down: i32, right: i32, up: i32) -> Rectangle {
        Rectangle::new(left, down, right, up)
    }

    #[test]
    fn rect_contain_clamps_overflowing_axis() {
        // Content taller than the viewport: it scrolls and stays within bounds.
        let viewport = rect(0, 0, 100, 100);
        let extend = UVec2::new(100, 300);
        let min = i64::q32_from_i32(100 - 300);
        let max = i64::q32_from_i32(0);

        assert_eq!(rect_contain(q(0, 0), extend, viewport).y, max);
        assert_eq!(rect_contain(q(0, -300), extend, viewport).y, min);
        // Overscrolling past the ends is clamped.
        assert_eq!(rect_contain(q(0, -1000), extend, viewport).y, min);
    }

    #[test]
    fn rect_contain_pins_undersized_axis() {
        // Content shorter than the viewport is pinned to the top / left.
        let viewport = rect(10, 20, 110, 120);
        let extend = UVec2::new(50, 40);

        assert_eq!(rect_contain(q(0, 0), extend, viewport), q(10, 80));
    }

    #[test]
    fn compose_center_nests_additively() {
        let parent = q(100, -50);
        let outer_scroll = q(10, 20);
        let inner_scroll = q(3, 4);

        let outer_center = compose_center(parent, outer_scroll);
        let inner_center = compose_center(outer_center, inner_scroll);

        assert_eq!(inner_center, q(100 - 10 - 3, -50 - 20 - 4));
    }
}
