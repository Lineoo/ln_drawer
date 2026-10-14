use glam::{I64Vec2, IVec2};
use ln_world::{Descriptor, Element, Handle, HandleGeneric, World};

use crate::{
    layout::transform::TransformValue,
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle, ScissorRect},
    render::{
        Render, RenderDrawControl, RenderExtra,
        camera::{Camera, CameraDescriptor, SetCameraOutdated},
    },
    theme::Theme,
    tools::{
        collider::{ToolControl, ToolHitResult},
        pointer::{PointerHit, PointerHitStatus, PointerScroll},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible, common::WidgetCommon,
        renderer::rrect::RRect,
    },
};

pub struct ContainerWithCamera {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub inner: TransformValue,
    pub scroll: I64Vec2,
}

impl Descriptor for ContainerWithCamera {
    type Target = (Handle<Container>, Handle<Camera>);

    fn when_build(self, world: &World) -> Self::Target {
        let parent = world.fetch(self.common.camera);
        let camera = world.insert(world.build(CameraDescriptor {
            center: parent.center,
            zoom: parent.zoom,
        }));

        let container = world.insert(Container {
            common: self.common,
            child: camera,
            rect: self.rect,
            inner: self.inner,
            scroll: self.scroll,
        });

        (container, camera)
    }
}

/// separated coordination
pub struct Container {
    pub common: WidgetCommon,
    pub child: Handle<Camera>,
    pub rect: Rectangle,
    pub inner: TransformValue,
    /// raw data to keep precision
    pub scroll: I64Vec2,
}

impl Container {
    pub fn init(&mut self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let parent_camera = world.fetch(self.common.camera);
        let mut child_camera = world.fetch_mut(self.child);

        let back = world.insert(RRect {
            rect: self.rect,
            order: 0,
            color: theme.primary_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let control = world.insert(RenderDrawControl {
            common: self.common,
            order: 100,
            draw: Box::new(move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>();
                let this = world.fetch(handle);
                let child = world.fetch(this.child);

                let camera = extra.camera;
                let panel_rect = this.rect;
                let window_size = lnwindow.window.surface_size();

                let left_up = lnwindow.screen_to_cursor(
                    camera.world_to_screen(I64Vec2::q32_from_i32(panel_rect.left_up())),
                );
                let right_down = lnwindow.screen_to_cursor(
                    camera.world_to_screen(I64Vec2::q32_from_i32(panel_rect.right_down())),
                );

                let x = (left_up.x.max(0.0) as u32).min(window_size.width);
                let y = (left_up.y.max(0.0) as u32).min(window_size.height);
                let right = (right_down.x.max(0.0) as u32).min(window_size.width);
                let down = (right_down.y.max(0.0) as u32).min(window_size.height);

                let clip = ScissorRect {
                    x,
                    y,
                    width: right.saturating_sub(x),
                    height: down.saturating_sub(y),
                };

                let clipped = match extra.scissor.intersect(clip) {
                    Some(result) => result,
                    // nothing to draw here, skip
                    None => return,
                };

                rpass.set_scissor_rect(clipped.x, clipped.y, clipped.width, clipped.height);
                Render::draw(
                    &child,
                    world,
                    rpass,
                    RenderExtra {
                        scissor: clipped,
                        ..extra
                    },
                );

                // revert
                rpass.set_scissor_rect(
                    extra.scissor.x,
                    extra.scissor.y,
                    extra.scissor.width,
                    extra.scissor.height,
                );
            }),
        });

        let tool = world.insert(ToolControl {
            common: self.common,
            order: isize::MAX,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(handle);
                let parent = world.fetch(this.common.camera);
                let position = parent.screen_to_world(screen);
                if this.rect.contains(position.q32_floor()) {
                    if let Some(hit) = ToolControl::intersect(world, screen, this.child) {
                        Some(hit)
                    } else {
                        Some(ToolHitResult {
                            camera: this.child,
                            handle: handle.untyped(),
                        })
                    }
                } else {
                    None
                }
            }),
        });

        world.dependency(back, handle);
        world.dependency(self.child, handle);
        world.dependency(handle, self.common.camera);
        world.dependency(control, handle);
        world.dependency(tool, handle);

        world.observer(self.common.camera, move |SetCameraOutdated, world| {
            let mut this = world.fetch_mut(handle);
            let parent_camera = world.fetch(this.common.camera);
            let mut child_camera = world.fetch_mut(this.child);
            this.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(this.child, SetCameraOutdated);
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
                    let mut this = world.fetch_mut(handle);
                    let parent_camera = world.fetch(this.common.camera);
                    let mut child_camera = world.fetch_mut(this.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);
                    this.scroll -= delta;
                    this.clamp_scroll();
                    this.update_camera(&parent_camera, &mut child_camera);
                    world.queue_trigger(this.child, SetCameraOutdated);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    let mut this = world.fetch_mut(handle);
                    let parent_camera = world.fetch(this.common.camera);
                    let mut child_camera = world.fetch_mut(this.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);
                    this.scroll -= delta;
                    this.clamp_scroll();
                    this.update_camera(&parent_camera, &mut child_camera);
                    world.queue_trigger(this.child, SetCameraOutdated);
                    None
                }
            }
        });

        world.observer(handle, move |event: &PointerScroll, world| {
            let mut this = world.fetch_mut(handle);
            let parent_camera = world.fetch(this.common.camera);
            let mut child_camera = world.fetch_mut(this.child);

            this.scroll -= I64Vec2::q32_from_f64(event.delta);
            this.clamp_scroll();
            this.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle);
            let parent_camera = world.fetch(this.common.camera);
            let mut child_camera = world.fetch_mut(this.child);

            let old = this.inner_rect();
            this.rect = rect;
            let new = this.inner_rect();

            this.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);

            world.queue_trigger(back, SetWidgetRectangle(rect));
            if old != new {
                world.queue_trigger(handle, WidgetRectangle(new));
            }
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle);
            this.common.visible = enabled;
            world.queue_trigger(tool, SetWidgetVisible(enabled));
            world.queue_trigger(control, SetWidgetVisible(enabled));
            world.queue_trigger(back, SetWidgetVisible(enabled));
            world.queue_trigger(handle, WidgetVisible(enabled));
        });

        self.clamp_scroll();
        self.update_camera(&parent_camera, &mut child_camera);
        world.queue_trigger(self.child, SetCameraOutdated);
    }

    fn inner_rect(&self) -> Rectangle {
        let zero_aligned = Rectangle {
            origin: IVec2::ZERO,
            extend: self.rect.extend,
        };
        self.inner.compute(zero_aligned)
    }

    fn clamp_scroll(&mut self) {
        let inner = self.inner_rect();
        let view = self.rect.extend;

        let x = match view.x < inner.width() {
            true => {
                let min = i64::q32_from_i32(inner.left());
                let max = i64::q32_from_i32(inner.right() - view.x as i32);
                self.scroll.x.clamp(min, max)
            }
            false => i64::q32_from_i32(inner.left()),
        };
        let y = match view.y < inner.height() {
            true => {
                let min = i64::q32_from_i32(inner.down());
                let max = i64::q32_from_i32(inner.up() - view.y as i32);
                self.scroll.y.clamp(min, max)
            }
            false => i64::q32_from_i32(inner.down()),
        };

        self.scroll = I64Vec2::new(x, y);
    }

    fn update_camera(&mut self, parent_camera: &Camera, child_camera: &mut Camera) {
        child_camera.center =
            parent_camera.center + self.scroll - I64Vec2::q32_from_i32(self.rect.origin)
    }
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
