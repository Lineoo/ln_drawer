use crate::{
    elements::menu::{MenuDescriptor, MenuEntryDescriptor},
    lnwin::PointerEvent,
    measures::{Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct TransformTool {
    active: Option<Handle<Transform>>,
    active_base: Option<Position>,
    pointer_base: Option<Position>,
}

pub struct Transform {
    pub rect: Rectangle,
    pub resizable: bool,
}

pub struct TransformUpdate;

impl Element for TransformTool {
    fn when_inserted(&mut self, world: &World, tool: Handle<Self>) {
        world.foreach_fetch::<Transform>(|transform, fetched_transform| {
            let collider = world.insert(PointerCollider {
                rect: fetched_transform.rect,
                z_order: ZOrder::new(50),
            });

            world.dependency(collider, tool);

            world.observer(collider, move |PointerHit(event), world, _| {
                let mut tool = world.fetch_mut(tool).unwrap();
                let mut fetched_transform = world.fetch_mut(transform).unwrap();
                match event {
                    PointerEvent::Pressed(position) => {
                        tool.active.replace(transform);
                        tool.active_base.replace(fetched_transform.rect.origin);
                        tool.pointer_base.replace(*position);
                    }
                    PointerEvent::Moved(position) => {
                        let delta = *position - tool.pointer_base.unwrap();
                        fetched_transform.rect.origin = tool.active_base.unwrap() + delta;
                        world.trigger(transform, TransformUpdate);
                    }
                    PointerEvent::Released(_) => {}
                }
            });

            world.observer(collider, move |&PointerMenu(position), world, _| {
                world.build(MenuDescriptor {
                    position,
                    entries: vec![
                        MenuEntryDescriptor {
                            label: "Reposition to World Original Point".into(),
                            action: Box::new(move |world| {
                                let mut fetched = world.fetch_mut(transform).unwrap();
                                fetched.rect.origin = Position::default();
                                world.trigger(transform, TransformUpdate);
                            }),
                        },
                        MenuEntryDescriptor {
                            label: "Stop Transform Tool".into(),
                            action: Box::new(move |world| {
                                world.remove(tool);
                            }),
                        },
                    ],
                    ..Default::default()
                });
            });

            let track = world.observer(transform, move |TransformUpdate, world, transform| {
                let fetched_transform = world.fetch(transform).unwrap();
                let mut collider = world.fetch_mut(collider).unwrap();
                collider.rect = fetched_transform.rect;
            });

            world.dependency(track, collider);
        });

        let main_collider = world.insert(PointerCollider::fullscreen(ZOrder::new(40)));

        world.observer(main_collider, move |&PointerMenu(position), world, _| {
            world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Stop Transform Tool".into(),
                    action: Box::new(move |world| {
                        world.remove(tool);
                    }),
                }],
                ..Default::default()
            });
        });

        world.dependency(main_collider, tool);
    }
}

impl Element for Transform {}
