use glam::{DVec2, Vec2};
use ln_world::{Element, Handle, HandleAny, World};

use crate::{
    layout::val::{Val, ValChanged},
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::camera::{Camera, UICamera},
    widgets::common::WidgetCommon,
};

pub struct ToolControl {
    pub common: WidgetCommon,
    pub order: isize,
    pub hit: Box<dyn Fn(&World, DVec2) -> Option<ToolHitResult> + Send + Sync>,
}

#[derive(Clone, Copy)]
pub struct ToolHitResult {
    pub camera: Handle<Camera>,
    pub handle: HandleAny,
}

/// trigger on lnwindow
pub struct ToolControlUpdate;

impl ToolControl {
    pub fn intersect_main(world: &World, screen: DVec2) -> Option<ToolHitResult> {
        let ui = world.single_fetch::<UICamera>().0;

        if let Some(value) = Self::intersect(world, screen, ui) {
            return Some(value);
        }

        None
    }

    pub fn intersect(
        world: &World,
        screen: DVec2,
        camera: Handle<Camera>,
    ) -> Option<ToolHitResult> {
        let camera = world.fetch(camera);
        for &control in camera.colliders.iter().rev() {
            let control = world.fetch(control);
            if world.fetch(control.common.visible).val
                && let Some(hit) = (control.hit)(world, screen)
            {
                return Some(hit);
            }
        }

        None
    }
}

impl Element for ToolControl {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("tool_control");
        world.set_autodeps(Some(this));

        world.dependency(this, self.common.visible);
        world.dependency(this, self.common.camera);
        let mut camera = world.fetch_mut(self.common.camera);
        camera.colliders.push(this);
        camera
            .colliders
            .sort_by_key(|&collider| match collider == this {
                true => self.order,
                false => world.fetch(collider).order,
            });
        world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        world.observer(self.common.visible, move |ValChanged, world| {
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let mut camera = world.fetch_mut(self.common.camera);
        camera.colliders.retain(|&x| x != this);
        world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
    }
}

pub struct ToolRectCollider {
    pub common: WidgetCommon,
    pub target: HandleAny,
    pub rect: Handle<Val<Rectangle>>,
    pub order: isize,
}

impl Element for ToolRectCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("tool_rect_collider");
        world.set_autodeps(Some(this));

        world.dependency(this, self.common.camera);
        world.dependency(this, self.common.visible);
        world.dependency(this, self.target);
        world.dependency(this, self.rect);

        world.insert(ToolControl {
            common: self.common,
            order: self.order,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(this);
                let camera = world.fetch(this.common.camera);
                let position = camera.screen_to_world(screen);
                let rect = world.fetch(this.rect).val;
                if rect.contains(position.q32_floor()) {
                    Some(ToolHitResult {
                        camera: this.common.camera,
                        handle: this.target,
                    })
                } else {
                    None
                }
            }),
        });

        world.observer(self.common.visible, move |ValChanged, world| {
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
        world.observer(self.rect, move |ValChanged, world| {
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }
}

/// where `0.5` touches the rect
pub struct ToolRingCollider {
    pub common: WidgetCommon,
    pub target: HandleAny,
    pub rect: Handle<Val<Rectangle>>,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub order: isize,
}

impl Element for ToolRingCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("tool_ring_collider");
        world.set_autodeps(Some(this));

        world.insert(ToolControl {
            common: self.common,
            order: self.order,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(this);
                let camera = world.fetch(this.common.camera);
                let position = camera.screen_to_world(screen);
                let rect = world.fetch(this.rect).val;
                let delta = (position.q32_floor() - rect.origin).as_vec2() / rect.extend.as_vec2()
                    - Vec2::splat(0.5);
                let radius = delta.length();

                if radius >= this.inner_radius && radius <= this.outer_radius {
                    Some(ToolHitResult {
                        camera: this.common.camera,
                        handle: this.target,
                    })
                } else {
                    None
                }
            }),
        });

        world.observer(self.common.visible, move |ValChanged, world| {
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
        world.observer(self.rect, move |ValChanged, world| {
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }
}
