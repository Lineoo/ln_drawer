use glam::{DVec2, I64Vec2, UVec2};
use ln_world::{Descriptor, Element, Handle, HandleGeneric, World};

use crate::{
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle, ScissorRect},
    render::{
        Render, RenderDrawControl, RenderExtra,
        camera::{Camera, CameraDescriptor, CameraOutdated, SetCameraOutdated},
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

pub struct ContainerAttr {
    /// where is the zero point on outer rectangle
    pub anchor: DVec2,
    /// offset insider rect
    pub scroll: I64Vec2,
    /// whether scissor rect should be set
    pub clip: bool,
    /// whether to ignore parent camera's linear transform
    pub top: bool,
}

impl Default for ContainerAttr {
    fn default() -> Self {
        Self {
            anchor: DVec2::new(0.0, 1.0),
            scroll: I64Vec2::ZERO,
            clip: true,
            top: false,
        }
    }
}

pub struct ContainerWithCamera {
    pub common: WidgetCommon,
    /// outer rectangle
    pub rect: Rectangle,
    pub attr: ContainerAttr,
}

impl Descriptor for ContainerWithCamera {
    // TODO return WidgetCommon
    type Target = (Handle<Container>, Handle<Camera>);

    fn when_build(self, world: &World) -> Self::Target {
        let parent = world.fetch(self.common.camera);
        let camera = world.insert(world.build(CameraDescriptor {
            center: parent.center,
            zoom: 1,
        }));

        let container = world.insert(Container {
            common: self.common,
            order: 100,
            child: camera,
            rect: self.rect,
            attr: self.attr,
        });

        (container, camera)
    }
}

/// separated coordination
pub struct Container {
    pub common: WidgetCommon,
    pub order: isize,
    pub child: Handle<Camera>,
    /// outer rectangle
    pub rect: Rectangle,
    pub attr: ContainerAttr,
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, handle: Handle<Self>) {
        let control = world.insert(RenderDrawControl {
            common: self.common,
            order: self.order,
            draw: Box::new(move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>();
                let this = world.fetch(handle);
                let child = world.fetch(this.child);

                if this.attr.clip {
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
                } else {
                    Render::draw(&child, world, rpass, extra);
                }
            }),
        });

        let tool = world.insert(ToolControl {
            common: self.common,
            order: self.order,
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

        world.dependency(self.child, handle);
        world.dependency(handle, self.common.camera);
        world.dependency(control, handle);
        world.dependency(tool, handle);

        world.observer(self.common.camera, move |CameraOutdated, world| {
            let this = world.fetch(handle);
            world.queue_trigger(this.child, SetCameraOutdated);
        });

        world.observer(self.child, move |SetCameraOutdated, world| {
            let this = world.fetch(handle);
            let lnwindow = world.single_fetch::<Lnwindow>();
            let parent_camera = world.fetch(this.common.camera);
            let mut child_camera = world.fetch_mut(this.child);
            let size = lnwindow.window.surface_size();

            this.update_camera(&parent_camera, &mut child_camera);
            child_camera.surface_size = UVec2::new(size.width, size.height);
            child_camera.upload();
            lnwindow.window.request_redraw();

            world.queue_trigger(this.child, CameraOutdated);
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle);
            this.rect = rect;
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle);
            this.common.visible = enabled;
            world.queue_trigger(tool, SetWidgetVisible(enabled));
            world.queue_trigger(control, SetWidgetVisible(enabled));
        });
    }
}

impl Container {
    fn update_camera(&self, parent_camera: &Camera, child_camera: &mut Camera) {
        child_camera.global_center = parent_camera.global_center + child_camera.center
            - self.rect.q32_anchor(self.attr.anchor)
            + self.attr.scroll;
        if !self.attr.top {
            child_camera.global_zoom = parent_camera.global_zoom + child_camera.zoom;
        } else {
            child_camera.global_zoom = child_camera.zoom;
        }
    }

    fn clamp_scroll(&mut self, restrict: Rectangle) {
        let min = I64Vec2::q32_from_i32(self.rect.left_down());
        let max = I64Vec2::q32_from_i32(self.rect.right_up());
        let center = self.rect.q32_anchor(self.attr.anchor);
        let min = min - center;
        let max = max - center;
        let x = match self.rect.width() < restrict.width() {
            true => {
                let min = i64::q32_from_i32(restrict.left()) - min.x;
                let max = i64::q32_from_i32(restrict.right()) - max.x;
                self.attr.scroll.x.clamp(min, max)
            }
            false => 0,
        };
        let y = match self.rect.height() < restrict.height() {
            true => {
                let min = i64::q32_from_i32(restrict.down()) - min.y;
                let max = i64::q32_from_i32(restrict.up()) - max.y;
                self.attr.scroll.y.clamp(min, max)
            }
            false => 0,
        };

        self.attr.scroll = I64Vec2::new(x, y);
    }
}

/// - emit inner area when scroll is zero in [`WidgetRectangle`] for container
pub struct ContainerEcho {
    pub container: Handle<Container>,
}

impl Element for ContainerEcho {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let handle = self.container;
        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let container = world.fetch(handle);
            let min = I64Vec2::q32_from_i32(rect.left_down());
            let max = I64Vec2::q32_from_i32(rect.right_up());
            let center = rect.q32_anchor(container.attr.anchor);
            let min = (min - center).q32_round();
            let max = (max - center).q32_round();
            world.queue_trigger(handle, WidgetRectangle(Rectangle::new_minmax(min, max)));
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            world.queue_trigger(handle, WidgetVisible(enabled));
        });

        world.dependency(this, self.container);
    }
}

/// - pointer scroll interaction
/// - add a non-shadow panel
/// - restrict are for scroll, and will emit the restriction area in [`WidgetRectangle`] for container
pub struct ContainerDeco {
    pub container: Handle<Container>,
    pub restriction: Rectangle,
}

impl Element for ContainerDeco {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let handle = self.container;
        let mut container = world.fetch_mut(self.container);
        let parent_camera = world.fetch(container.common.camera);
        let mut child_camera = world.fetch_mut(container.child);

        let back = world.insert(RRect {
            rect: container.rect,
            order: 10,
            color: theme.gray1,
            radius: theme.roundness,
            width: 0.0,
            enabled: container.common.visible,
            camera: container.common.camera,
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
                    let this = world.fetch(this);
                    let mut container = world.fetch_mut(handle);
                    let parent_camera = world.fetch(container.common.camera);
                    let mut child_camera = world.fetch_mut(container.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);

                    container.attr.scroll -= delta;
                    container.clamp_scroll(this.restriction);
                    container.update_camera(&parent_camera, &mut child_camera);
                    world.queue_trigger(container.child, SetCameraOutdated);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    let this = world.fetch(this);
                    let mut container = world.fetch_mut(handle);
                    let parent_camera = world.fetch(container.common.camera);
                    let mut child_camera = world.fetch_mut(container.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);

                    container.attr.scroll -= delta;
                    container.clamp_scroll(this.restriction);
                    container.update_camera(&parent_camera, &mut child_camera);
                    world.queue_trigger(container.child, SetCameraOutdated);
                    None
                }
            }
        });

        world.observer(handle, move |event: &PointerScroll, world| {
            let this = world.fetch(this);
            let mut container = world.fetch_mut(handle);
            let parent_camera = world.fetch(container.common.camera);
            let mut child_camera = world.fetch_mut(container.child);

            let delta = I64Vec2::q32_from_f64(event.delta);
            container.attr.scroll -= delta;
            container.clamp_scroll(this.restriction);
            container.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let this = world.fetch(this);
            let mut container = world.fetch_mut(handle);
            let parent_camera = world.fetch(container.common.camera);
            let mut child_camera = world.fetch_mut(container.child);

            container.rect = rect;
            container.clamp_scroll(this.restriction);
            container.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
            world.queue_trigger(back, SetWidgetRectangle(rect));
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle);
            this.common.visible = enabled;
            world.queue_trigger(handle, WidgetVisible(enabled));
            world.queue_trigger(back, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            let mut container = world.fetch_mut(handle);
            let parent_camera = world.fetch(container.common.camera);
            let mut child_camera = world.fetch_mut(container.child);
            this.restriction = rect;

            container.clamp_scroll(this.restriction);
            container.update_camera(&parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
            world.queue_trigger(handle, WidgetRectangle(this.restriction));
        });

        container.clamp_scroll(self.restriction);
        container.update_camera(&parent_camera, &mut child_camera);
        world.queue_trigger(container.child, SetCameraOutdated);

        world.dependency(back, this);
        world.dependency(this, self.container);
    }
}

/// auto setup ContainerDeco's restriction area based on Container's view rect
pub struct ContainerDecoDropdown {
    pub height: Option<i32>,
    pub container: Handle<Container>,
    pub deco: Handle<ContainerDeco>,
}

impl Element for ContainerDecoDropdown {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        let deco = self.deco;
        let height = self.height;
        world.observer(self.container, move |SetWidgetRectangle(rect), world| {
            world.queue_trigger(
                deco,
                SetWidgetRectangle(Rectangle::new(
                    0,
                    -height.unwrap_or(rect.extend.y as i32),
                    rect.extend.x as i32,
                    0,
                )),
            );
        });
    }
}
