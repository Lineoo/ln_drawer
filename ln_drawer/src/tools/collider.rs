use glam::DVec2;
use ln_world::{Element, Handle, HandleAny, World};

use crate::{
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::camera::{Camera, UICamera},
    widgets::{SetWidgetRectangle, SetWidgetVisible, common::WidgetCommon},
};

pub struct ToolControl {
    pub common: WidgetCommon,
    pub order: isize,
    pub hit: Box<dyn Fn(&World, DVec2) -> Option<ToolHitResult> + Send + Sync>,
}

pub struct ToolRectCollider {
    pub common: WidgetCommon,
    pub target: HandleAny,
    pub rect: Rectangle,
    pub order: isize,
}

#[derive(Clone, Copy)]
pub struct ToolHitResult {
    pub camera: Handle<Camera>,
    pub handle: HandleAny,
}

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
            if control.common.visible
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
        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = visible;
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let mut camera = world.fetch_mut(self.common.camera);
        camera.colliders.retain(|&x| x != this);
        world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
    }
}

impl Element for ToolRectCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(ToolControl {
            common: self.common,
            order: self.order,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(this);
                let camera = world.fetch(this.common.camera);
                let position = camera.screen_to_world(screen);
                if this.rect.contains(position.q32_floor()) {
                    Some(ToolHitResult {
                        camera: this.common.camera,
                        handle: this.target,
                    })
                } else {
                    None
                }
            }),
        });

        world.dependency(control, this);
        world.dependency(this, self.common.camera);
        world.dependency(this, self.target);
        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = visible;
            world.queue_trigger(control, SetWidgetVisible(visible));
        });
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }
}
