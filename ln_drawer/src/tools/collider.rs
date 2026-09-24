use glam::{DVec2, I64Vec2, IVec2, UVec2};
use ln_world::{Element, Handle, World};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::{Camera, MainCamera, UICamera},
    widgets::{SetWidgetRectangle, container::Container},
};

#[derive(Clone, Copy)]
pub struct ToolCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
    /// The camera this collider lives in; drive the whole hit test through the camera tree.
    pub camera: Handle<Camera>,
}

/// The topmost [`ToolCollider`] under a screen point.
///
/// `camera` is the collider's own camera (its local coordinate system) and `view` is the root ECS
/// view it lives in, so callers can enter `view` to deliver events. `position` is `screen`
/// converted into `camera`'s world space.
#[derive(Clone, Copy)]
pub struct ToolHit {
    pub collider: Handle<ToolCollider>,
    pub camera: Handle<Camera>,
    pub position: I64Vec2,
}

/// Event node for [`ToolColliderChanged`]
pub struct ToolColliderDispatcher;
pub struct ToolColliderChanged(pub Handle<ToolCollider>);

impl ToolCollider {
    pub const fn fullscreen(order: isize, camera: Handle<Camera>) -> ToolCollider {
        ToolCollider {
            rect: Rectangle {
                origin: IVec2::MIN,
                extend: UVec2::MAX,
            },
            order,
            enabled: true,
            camera,
        }
    }

    /// Find the topmost collider under `screen`, called from the window view.
    ///
    /// The search mirrors the camera tree instead of ECS views. It walks the root cameras from top
    /// to bottom, and inside each camera it first descends into nested [`Container`] cameras
    /// (their contents draw last, so they sit above their siblings) before testing the camera's
    /// own colliders by `order`. The clip rectangle is narrowed at every container on the way down,
    /// so content scrolled or clipped outside a container no longer reacts to the pointer, and
    /// hidden containers hide their whole subtree.
    pub fn intersect(world: &World, screen: DVec2) -> Option<ToolHit> {
        let main = world.single_fetch::<MainCamera>().ok()?.0;
        let ui = world.single_fetch::<UICamera>().ok()?.0;

        let mut visited = Vec::new();
        // UI draws over the painting canvas, so test it first.
        for camera in [ui, main] {
            if let Some(hit) = hit_camera(world, camera, screen, HitClip::FULL, &mut visited) {
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
    let low = camera.src_to_dst(I64Vec2::q32_from_i32(rect.left_down()));
    let high = camera.src_to_dst(I64Vec2::q32_from_i32(rect.right_up()));

    HitClip {
        min: low.min(high),
        max: low.max(high),
    }
}

/// Recursively test one camera, topmost collider first.
///
/// `view` is the root ECS view the whole camera subtree lives in; entering it makes the cameras,
/// containers and colliders of this subtree visible.
fn hit_camera(
    world: &World,
    camera: Handle<Camera>,
    screen: DVec2,
    clip: HitClip,
    visited: &mut Vec<Handle<Camera>>,
) -> Option<ToolHit> {
    if !clip.contains(screen) || visited.contains(&camera) {
        return None;
    }
    visited.push(camera);

    let camera_ref = world.fetch(camera).ok()?;

    // Nested containers draw after the rest of the camera, so their content sits on top.
    let mut containers = Vec::new();
    world.foreach_fetch::<Container>(|container| {
        if container.parent == camera {
            containers.push(container.handle());
        }
    });

    for container in containers.into_iter().rev() {
        let Ok(entry) = world.fetch(container) else {
            continue;
        };
        let visible = entry.visible;
        let rect = entry.rect;
        let child_camera = entry.camera;
        drop(entry);

        if !visible {
            continue;
        }

        let child = clip.intersect(projected(&camera_ref, rect));
        if let Some(hit) = hit_camera(world, child_camera, screen, child, visited) {
            return Some(hit);
        }
    }

    // Then the camera's own colliders: highest `order`, and latest inserted, first.
    let position = camera_ref.dst_to_src(screen);
    let flat = position.q32_floor();

    let mut colliders = Vec::new();
    world.foreach_fetch::<ToolCollider>(|collider| {
        if collider.camera == camera {
            colliders.push((
                collider.order,
                collider.handle(),
                collider.rect,
                collider.enabled,
            ));
        }
    });
    colliders.sort_by_key(|entry| entry.0);

    for (_, collider, rect, enabled) in colliders.into_iter().rev() {
        if enabled && rect.contains(flat) {
            return Some(ToolHit {
                collider,
                camera,
                position,
            });
        }
    }

    None
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
