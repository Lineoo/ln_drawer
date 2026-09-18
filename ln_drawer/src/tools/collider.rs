use glam::{DVec2, I64Vec2, IVec2, UVec2};
use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::{Camera, CurrentCamera},
    widgets::{SetWidgetRectangle, container::Container},
};

#[derive(Clone, Copy)]
pub struct ToolCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

/// The topmost [`ToolCollider`] under a screen point.
///
/// `view` is the view the collider lives in; `position` is `screen` converted into that view's
/// world space, so callers do not have to look the camera up again.
#[derive(Clone, Copy)]
pub struct ToolHit {
    pub collider: Handle<ToolCollider>,
    pub view: HandleAny,
    pub position: I64Vec2,
}

/// Event node for [`ToolColliderChanged`]
pub struct ToolColliderDispatcher;
pub struct ToolColliderChanged(pub Handle<ToolCollider>);

impl ToolCollider {
    pub const fn fullscreen(order: isize) -> ToolCollider {
        ToolCollider {
            rect: Rectangle {
                origin: IVec2::MIN,
                extend: UVec2::MAX,
            },
            order,
            enabled: true,
        }
    }

    /// Find the topmost collider under `screen`, called from the window view.
    ///
    /// The search mirrors the render hierarchy instead of flattening every camera into a portal.
    /// It walks the root cameras from top to bottom, and inside each view it first descends into
    /// nested [`Container`] views (their contents draw last, so they sit above their siblings)
    /// before testing the view's own colliders by `order`. The clip rectangle is narrowed at every
    /// container on the way down, so content scrolled or clipped outside a container no longer
    /// reacts to the pointer, and hidden containers hide their whole subtree.
    pub fn intersect(world: &World, screen: DVec2) -> Option<ToolHit> {
        let mut roots = Vec::new();
        world.foreach_fetch::<Camera>(|camera| roots.push(camera.handle()));

        let mut visited = Vec::new();
        for camera in roots.into_iter().rev() {
            if let Some(hit) =
                hit_view(world, camera.untyped(), screen, HitClip::FULL, &mut visited)
            {
                return Some(hit);
            }
        }

        None
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });
    }
}

/// A screen-space clip rectangle in normalized device coordinates.
#[derive(Clone, Copy)]
struct HitClip {
    min: DVec2,
    max: DVec2,
}

impl HitClip {
    const FULL: HitClip = HitClip {
        min: DVec2::splat(-1.0),
        max: DVec2::splat(1.0),
    };

    fn contains(self, point: DVec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    fn intersect(self, other: HitClip) -> HitClip {
        HitClip {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }
}

/// Project a rectangle from `camera`'s world space into normalized device coordinates.
fn projected(camera: &Camera, rect: Rectangle) -> HitClip {
    let low = camera.world_to_screen_absolute(I64Vec2::q32_from_i32(rect.left_down()));
    let high = camera.world_to_screen_absolute(I64Vec2::q32_from_i32(rect.right_up()));

    HitClip {
        min: low.min(high),
        max: low.max(high),
    }
}

/// Recursively test one view, topmost collider first.
fn hit_view(
    world: &World,
    view: HandleAny,
    screen: DVec2,
    clip: HitClip,
    visited: &mut Vec<HandleAny>,
) -> Option<ToolHit> {
    if !clip.contains(screen) || visited.contains(&view) {
        return None;
    }
    visited.push(view);

    world.enter(view, || {
        let Ok(current) = world.single_fetch::<CurrentCamera>() else {
            return None;
        };
        let camera = world.fetch(current.0).ok()?;

        // Nested containers draw after the rest of the view, so their content sits on top.
        let mut containers = Vec::new();
        world.foreach_fetch::<Container>(|container| containers.push(container.handle()));

        for container in containers.into_iter().rev() {
            let Ok(entry) = world.fetch(container) else {
                continue;
            };
            let visible = entry.visible;
            let rect = entry.rect;
            drop(entry);

            if !visible {
                continue;
            }

            let child = clip.intersect(projected(&camera, rect));
            if let Some(hit) = hit_view(world, container.untyped(), screen, child, visited) {
                return Some(hit);
            }
        }

        // Then the view's own colliders: highest `order`, and latest inserted, first.
        let position = camera.screen_to_world_absolute(screen);
        let flat = position.q32_floor();

        let mut colliders = Vec::new();
        world.foreach_fetch::<ToolCollider>(|collider| {
            colliders.push((
                collider.order,
                collider.handle(),
                collider.rect,
                collider.enabled,
            ));
        });
        colliders.sort_by_key(|entry| entry.0);

        for (_, collider, rect, enabled) in colliders.into_iter().rev() {
            if enabled && rect.contains(flat) {
                return Some(ToolHit {
                    collider,
                    view,
                    position,
                });
            }
        }

        None
    })
}

impl Element for ToolCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_layout(world, this);
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.queue_trigger(dispatcher, ToolColliderChanged(this));
        world.dependency(this, dispatcher);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.queue_trigger(dispatcher, ToolColliderChanged(this));
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.queue_trigger(dispatcher, ToolColliderChanged(this));
    }
}

impl Element for ToolColliderDispatcher {}
