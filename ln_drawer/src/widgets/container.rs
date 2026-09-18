use glam::{DVec2, I64Vec2};
use ln_world::{ElemRef, Element, Handle, HandleGeneric, World};

use crate::{
    layout::transform::TransformValue,
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{
        Render,
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

        world.enter_queue(handle, move |world| {
            world.insert(ElemRef(lnwindow.untyped()));
            world.queue(move |world| {
                let camera = world.insert(camera);
                world.insert(CurrentCamera(camera));
            });
        });

        let mut status = None;
        world.observer(collider, move |event: &PointerHit, world| {
            status = match (status, event.status) {
                (None, PointerHitStatus::Press | PointerHitStatus::Moving) => {
                    Some(event.pointer.screen)
                }
                (None, PointerHitStatus::Release) => None,
                (Some(_), PointerHitStatus::Press) => Some(event.pointer.screen),
                (Some(position), PointerHitStatus::Moving) => {
                    move_camera(world, handle, event.pointer.screen - position);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    move_camera(world, handle, event.pointer.screen - position);
                    None
                }
            }
        });

        world.observer(collider, move |event: &PointerScroll, world| {
            move_camera(world, handle, event.delta);
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
            move_camera(world, handle, (rect.origin - old_origin).as_dvec2());
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
pub(crate) fn move_camera(world: &World, handle: Handle<Container>, delta: DVec2) {
    world.enter(handle, || {
        let current = world.single_fetch::<CurrentCamera>().unwrap();
        let mut camera = world.fetch_mut(current.0).unwrap();
        let delta = camera.screen_to_world_relative(delta).q32_round();

        let (inner, rect) = {
            let this = world.fetch(handle).unwrap();
            (this.inner, this.rect)
        };

        let position = -camera.center + I64Vec2::q32_from_i32(delta);
        let anchored = Rectangle {
            origin: position.q32_round(),
            extend: inner.extend,
        };
        camera.center = -I64Vec2::q32_from_i32(rect_contain(anchored, rect).origin);
    });
}

/// Keep `content` inside `viewport`.
///
/// Axes that overflow their viewport axis scroll normally. Axes that are too small to fill the
/// viewport cannot scroll, so the content is pinned to the top / left edge instead of drifting
/// freely (the conventional list behavior).
fn rect_contain(content: Rectangle, viewport: Rectangle) -> Rectangle {
    let width = content.width();
    let height = content.height();

    let left = match width >= viewport.width() {
        true => content
            .left()
            .clamp(viewport.right() - width as i32, viewport.left()),
        false => viewport.left(),
    };
    let down = match height >= viewport.height() {
        true => content
            .down()
            .clamp(viewport.up() - height as i32, viewport.down()),
        false => viewport.up() - height as i32,
    };

    Rectangle::new_extend(left, down, width, height)
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
