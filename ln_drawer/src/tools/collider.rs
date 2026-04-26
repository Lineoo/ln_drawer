use crate::{
    layout::LayoutRectangleAction,
    measures::{Position, Rectangle, Size},
    render::camera::Camera,
    world::{Element, Handle, World},
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

    pub fn intersect(
        world: &World,
        screen: [f64; 2],
    ) -> Vec<(Handle<ToolCollider>, Handle<Camera>)> {
        let mut buf = Vec::new();
        world.foreach_enter::<Camera>(|camera| {
            let camera = world.fetch(camera).unwrap();
            let position = camera.screen_to_world_absolute(screen).floor();
            world.foreach_fetch::<ToolCollider>(|collider| {
                if collider.enabled && position.within(collider.rect) {
                    buf.push((collider.handle(), camera.handle(), collider.order));
                }
            });
        });

        buf.sort_by(|(.., a), (.., b)| b.cmp(a));
        buf.iter().map(|x| (x.0, x.1)).collect::<Vec<_>>()
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        let action = LayoutRectangleAction(Box::new(move |world, rect| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
            rect
        }));

        world.enter_insert(this, action);
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
