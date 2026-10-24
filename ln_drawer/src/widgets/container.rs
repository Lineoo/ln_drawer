use glam::{DVec2, I64Vec2, UVec2};
use ln_world::{Descriptor, Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        transfer::Transfer,
        val::{Val, ValChanged},
    },
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
    widgets::{common::WidgetCommon, renderer::rrect::RRect},
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
    pub rect: Handle<Val<Rectangle>>,
    pub attr: ContainerAttr,
}

impl Descriptor for ContainerWithCamera {
    type Target = (Handle<Container>, WidgetCommon);

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

        let common = WidgetCommon {
            camera,
            visible: self.common.visible,
        };

        (container, common)
    }
}

/// separated coordination
pub struct Container {
    pub common: WidgetCommon,
    pub order: isize,
    pub child: Handle<Camera>,
    /// outer rectangle
    pub rect: Handle<Val<Rectangle>>,
    pub attr: ContainerAttr,
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, handle: Handle<Self>) {
        world.scope("container");
        world.set_autodeps(Some(handle));

        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);
        world.dependency(self.child, handle);
        world.dependency(handle, self.common.camera);

        world.insert(RenderDrawControl {
            common: self.common,
            order: self.order,
            draw: Box::new(move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>();
                let this = world.fetch(handle);
                let child = world.fetch(this.child);

                if this.attr.clip {
                    let camera = extra.camera;
                    let panel_rect = world.fetch(this.rect).val;
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

        world.insert(ToolControl {
            common: self.common,
            order: self.order,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(handle);
                let parent = world.fetch(this.common.camera);
                let position = parent.screen_to_world(screen);
                let rect = world.fetch(this.rect).val;
                if rect.contains(position.q32_floor()) {
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

        world.observer(self.common.camera, move |CameraOutdated, world| {
            let this = world.fetch(handle);
            world.queue_trigger(this.child, SetCameraOutdated);
        });

        world.observer(self.child, move |SetCameraOutdated, world| {
            let this = world.fetch(handle);
            let lnwindow = world.single_fetch::<Lnwindow>();
            let parent_camera = world.fetch(this.common.camera);
            let mut child_camera = world.fetch_mut(this.child);
            let rect = world.fetch(this.rect).val;
            let size = lnwindow.window.surface_size();

            this.update_camera(rect, &parent_camera, &mut child_camera);
            child_camera.surface_size = UVec2::new(size.width, size.height);
            child_camera.upload();
            lnwindow.window.request_redraw();

            world.queue_trigger(this.child, CameraOutdated);
        });

        world.observer(self.rect, move |ValChanged, world| {
            let this = world.fetch(handle);
            world.queue_trigger(this.child, SetCameraOutdated);
        });
    }
}

impl Container {
    fn update_camera(&self, rect: Rectangle, parent_camera: &Camera, child_camera: &mut Camera) {
        child_camera.global_center = parent_camera.global_center + child_camera.center
            - rect.q32_anchor(self.attr.anchor)
            + self.attr.scroll;
        if !self.attr.top {
            child_camera.global_zoom = parent_camera.global_zoom + child_camera.zoom;
        } else {
            child_camera.global_zoom = child_camera.zoom;
        }
    }

    fn clamp_scroll(&mut self, rect: Rectangle, restrict: Rectangle) {
        let min = I64Vec2::q32_from_i32(rect.left_down());
        let max = I64Vec2::q32_from_i32(rect.right_up());
        let center = rect.q32_anchor(self.attr.anchor);
        let min = min - center;
        let max = max - center;
        let x = match rect.width() < restrict.width() {
            true => {
                let min = i64::q32_from_i32(restrict.left()) - min.x;
                let max = i64::q32_from_i32(restrict.right()) - max.x;
                self.attr.scroll.x.clamp(min, max)
            }
            false => 0,
        };
        let y = match rect.height() < restrict.height() {
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

/// - inner area when scroll is zero for container
pub struct ContainerEcho {
    pub container: Handle<Container>,
}

impl Descriptor for ContainerEcho {
    type Target = Handle<Val<Rectangle>>;

    fn when_build(self, world: &World) -> Self::Target {
        let container = self.container;
        let inner = world.insert(Val::new(Rectangle::default()));

        world.queue(move |world| {
            world.scope("container_echo");
            world.set_autodeps(Some(container));

            let container = world.fetch(container);
            let container_rect = container.rect;
            let anchor = container.attr.anchor;
            world.observer(container_rect, move |ValChanged, world| {
                let rect = world.fetch(container_rect).val;
                let mut inner = world.fetch_mut(inner);

                let min = I64Vec2::q32_from_i32(rect.left_down());
                let max = I64Vec2::q32_from_i32(rect.right_up());
                let center = rect.q32_anchor(anchor);
                let min = (min - center).q32_round();
                let max = (max - center).q32_round();
                inner.val = Rectangle::new_minmax(min, max);
                world.queue_trigger(inner.handle(), ValChanged);
            });
        });

        inner
    }
}

/// - pointer scroll interaction
/// - add a non-shadow panel
/// - restrict is for scroll
pub struct ContainerDeco {
    pub container: Handle<Container>,
    pub restriction: Handle<Val<Rectangle>>,
}

impl Element for ContainerDeco {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("container_deco");
        world.set_autodeps(Some(this));

        let theme = world.single_fetch::<Theme>();
        let handle = self.container;
        let container = world.fetch(handle);

        world.insert(RRect {
            common: container.common,
            rect: container.rect,
            order: 10,
            color: world.insert(Val::new(theme.gray1)),
            radius: theme.roundness,
            width: 0.0,
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
                    let rect = world.fetch(container.rect).val;
                    let restriction = world.fetch(this.restriction).val;
                    let parent_camera = world.fetch(container.common.camera);
                    let mut child_camera = world.fetch_mut(container.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);

                    container.attr.scroll -= delta;
                    container.clamp_scroll(rect, restriction);
                    container.update_camera(rect, &parent_camera, &mut child_camera);
                    world.queue_trigger(container.child, SetCameraOutdated);
                    Some(event.pointer.screen)
                }
                (Some(position), PointerHitStatus::Release) => {
                    let this = world.fetch(this);
                    let mut container = world.fetch_mut(handle);
                    let rect = world.fetch(container.rect).val;
                    let restriction = world.fetch(this.restriction).val;
                    let parent_camera = world.fetch(container.common.camera);
                    let mut child_camera = world.fetch_mut(container.child);
                    let delta =
                        parent_camera.screen_to_world_relative(event.pointer.screen - position);

                    container.attr.scroll -= delta;
                    container.clamp_scroll(rect, restriction);
                    container.update_camera(rect, &parent_camera, &mut child_camera);
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
            let rect = world.fetch(container.rect).val;
            let restriction = world.fetch(this.restriction).val;

            let delta = I64Vec2::q32_from_f64(event.delta);
            container.attr.scroll -= delta;
            container.clamp_scroll(rect, restriction);
            container.update_camera(rect, &parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
        });

        world.observer(container.rect, move |ValChanged, world| {
            let this = world.fetch(this);
            let mut container = world.fetch_mut(handle);
            let parent_camera = world.fetch(container.common.camera);
            let mut child_camera = world.fetch_mut(container.child);
            let rect = world.fetch(container.rect).val;
            let restriction = world.fetch(this.restriction).val;

            container.clamp_scroll(rect, restriction);
            container.update_camera(rect, &parent_camera, &mut child_camera);
            world.queue_trigger(child_camera.handle(), SetCameraOutdated);
        });
    }
}

/// auto setup ContainerDeco's restriction area based on Container's view rect
pub struct ContainerDecoDropdown {
    pub height: Option<i32>,
    pub outer: Handle<Val<Rectangle>>,
}

impl Descriptor for ContainerDecoDropdown {
    type Target = Handle<Val<Rectangle>>;

    fn when_build(self, world: &World) -> Self::Target {
        Transfer::derive(world, self.outer).build(move |&rect| {
            Rectangle::new(
                0,
                -self.height.unwrap_or(rect.extend.y as i32),
                rect.extend.x as i32,
                0,
            )
        })
    }
}
