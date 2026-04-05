use crate::{
    layout::{LayoutControl, LayoutControls, LayoutRectangle},
    measures::{Position, Rectangle, Size},
    render::camera::{Camera, CameraVisits},
    widgets::WidgetRectangle,
    world::{Element, Handle, ViewId, World},
};

#[derive(Clone, Copy)]
pub struct ToolCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

/// Event node for [`ToolColliderChanged`]
pub struct ToolColliderDispatcher;
pub struct ToolColliderChanged(pub Handle<ToolCollider>);

impl ToolCollider {
    pub const fn fullscreen(order: isize) -> ToolCollider {
        ToolCollider {
            rect: Rectangle {
                origin: Position::MIN,
                extend: Size::MAX,
            },
            order,
            enabled: true,
        }
    }

    pub fn intersect(world: &World, screen: [f64; 2]) -> Vec<(Handle<ToolCollider>, ViewId)> {
        let mut buf = Vec::new();
        let visits = world.single_fetch::<CameraVisits>().unwrap();
        for &view in &visits.views {
            world.enter(view, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                let position = camera.screen_to_world_absolute(screen).floor();
                world.foreach_fetch::<ToolCollider>(|collider| {
                    if collider.enabled && position.within(collider.rect) {
                        buf.push((collider.handle(), view, collider.order));
                    }
                });
            });
        }

        buf.sort_by(|(.., a), (.., b)| b.cmp(a));
        buf.iter().map(|x| (x.0, x.1)).collect::<Vec<_>>()
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(LayoutControl {
            rectangle: Some(Box::new(move |world, rect| {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = rect;
                rect
            })),
            enabled: None,
        });

        let mut layouts = world.single_fetch_mut::<LayoutControls>().unwrap();
        layouts.0.insert(this.untyped(), control);
        world.dependency(control, this);
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

impl Element for ToolColliderDispatcher {}
