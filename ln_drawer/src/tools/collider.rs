use glam::{DVec2, IVec2, UVec2};
use ln_world::{Element, Handle, HandleAny, World};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::CurrentCamera,
    widgets::SetWidgetRectangle,
};

#[derive(Clone, Copy)]
pub struct ToolCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

pub struct ToolColliderPortal(pub HandleAny);

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

    pub fn intersect(world: &World, screen: DVec2) -> Vec<(Handle<ToolCollider>, HandleAny)> {
        let mut buf = Vec::new();
        world.foreach_fetch::<ToolColliderPortal>(|portal| {
            world.enter(portal.0, || {
                let camera = world.single_fetch::<CurrentCamera>().unwrap();
                let camera = world.fetch(camera.0).unwrap();
                let position = camera.screen_to_world_absolute(screen).q32_floor();
                world.foreach_fetch::<ToolCollider>(|collider| {
                    if collider.enabled && collider.rect.contains(position) {
                        buf.push((collider.handle(), portal.0, collider.order));
                    }
                });
            });
        });

        buf.sort_by(|(.., a), (.., b)| b.cmp(a));
        buf.iter().map(|x| (x.0, x.1)).collect::<Vec<_>>()
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });
    }
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

impl Element for ToolColliderPortal {}
impl Element for ToolColliderDispatcher {}
