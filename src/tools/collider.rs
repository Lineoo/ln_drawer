use crate::{
    layout::LayoutRectangle,
    measures::{Position, Rectangle, Size},
    tools::{mouse::MouseTool, pointer::PointerTool},
    world::{Element, Handle, World, WorldError},
};

#[derive(Clone, Copy)]
pub struct PointerCollider {
    pub rect: Rectangle,
    pub order: isize,
    pub enabled: bool,
}

/// Event node for [`ToolColliderChanged`]
pub struct ToolColliderDispatcher;
pub struct ToolColliderChanged(pub Handle<PointerCollider>);

impl PointerCollider {
    pub const fn fullscreen(order: isize) -> PointerCollider {
        PointerCollider {
            rect: Rectangle {
                origin: Position::MIN,
                extend: Size::MAX,
            },
            order,
            enabled: true,
        }
    }

    pub fn intersect(world: &World, point: Position) -> Vec<Handle<PointerCollider>> {
        let mut result = Vec::with_capacity(8);
        world.foreach_fetch::<PointerCollider>(|collider| {
            if collider.enabled && point.within(collider.rect) {
                result.push((collider.handle(), collider.order));
            }
        });

        result.sort_by(|(_, a), (_, b)| b.cmp(a));
        result.iter().map(|x| x.0).collect::<Vec<_>>()
    }
}

impl ToolColliderDispatcher {
    pub fn init(world: &mut World) {
        world.insert(ToolColliderDispatcher);
        world.flush();
    }
}

impl Element for PointerCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.trigger(dispatcher, &ToolColliderChanged(this));

        world.observer(this, move |&LayoutRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.trigger(dispatcher, &ToolColliderChanged(this));
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        world.trigger(dispatcher, &ToolColliderChanged(this));
    }
}

impl Element for ToolColliderDispatcher {}
