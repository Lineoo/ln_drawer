use crate::{
    elements::menu::{MenuDescriptor, MenuEntryDescriptor},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    render::wireframe::{Wireframe, WireframeDescriptor},
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Element, Handle, World},
};

pub struct Transform {
    pub rect: Rectangle,
    pub resizable: bool,
}

pub struct TransformUpdate;

impl Element for Transform {}

#[derive(Default)]
pub struct TransformTool {
    active: Option<Active>,
}

struct Active {
    target: Handle<Transform>,
    frame: Wireframe,
    resizing: Option<Vec<ResizeKnob>>,
    dragging: Option<Dragging>,
}

struct ResizeKnob {
    wireframe: Wireframe,
    collider: Handle<PointerCollider>,
    dragging: Option<Dragging>,
}

struct Dragging {
    element_base: Position,
    pointer_base: Position,
}

impl Element for TransformTool {
    fn when_inserted(&mut self, world: &World, tool: Handle<Self>) {
        world.foreach_fetch::<Transform>(|transform, fetched_transform| {
            let collider = world.insert(PointerCollider {
                rect: fetched_transform.rect,
                order: 50,
            });

            world.dependency(collider, tool);

            world.observer(collider, move |PointerHit(event), world, _| {
                let mut fetched_tool = world.fetch_mut(tool).unwrap();
                let mut fetched_transform = world.fetch_mut(transform).unwrap();
                match (event, &mut fetched_tool.active) {
                    (PointerEvent::Pressed(position), None) => {
                        let frame = world.build(WireframeDescriptor {
                            rect: fetched_transform.rect,
                            ..Default::default()
                        });

                        let old = fetched_tool.active.replace(Active {
                            target: transform,
                            frame,
                            dragging: Some(Dragging {
                                element_base: fetched_transform.rect.origin,
                                pointer_base: *position,
                            }),
                            resizing: None,
                        });

                        if let Some(old) = old
                            && let Some(resizing) = old.resizing
                        {
                            for knob in resizing {
                                world.remove(knob.collider);
                            }
                        }
                    }
                    (PointerEvent::Pressed(position), Some(active)) => {
                        active.dragging.replace(Dragging {
                            element_base: fetched_transform.rect.origin,
                            pointer_base: *position,
                        });
                    }
                    (PointerEvent::Moved(position), Some(active)) => {
                        if let Some(dragging) = &active.dragging {
                            let delta = *position - dragging.pointer_base;
                            fetched_transform.rect.origin = dragging.element_base + delta;

                            active.frame.rect = fetched_transform.rect;
                            active.frame.upload();

                            world.trigger(transform, TransformUpdate);
                        }
                    }
                    (PointerEvent::Released(_), Some(active)) => {
                        active.dragging = None;
                    }
                    _ => {
                        log::warn!("not expected state");
                    }
                }
            });

            world.observer(collider, move |&PointerMenu(position), world, _| {
                world.insert(world.build(MenuDescriptor {
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
                }));
            });

            let track = world.observer(transform, move |TransformUpdate, world, transform| {
                let fetched_transform = world.fetch(transform).unwrap();
                let mut collider = world.fetch_mut(collider).unwrap();
                collider.rect = fetched_transform.rect;
            });

            world.dependency(track, collider);
        });

        let main_collider = world.insert(PointerCollider::fullscreen(40));

        world.observer(main_collider, move |PointerHit(event), world, _| {
            let mut tool = world.fetch_mut(tool).unwrap();
            if let PointerEvent::Pressed(_) = event {
                tool.active = None;
            }
        });

        world.observer(main_collider, move |&PointerMenu(position), world, _| {
            world.insert(world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Stop Transform Tool".into(),
                    action: Box::new(move |world| {
                        world.remove(tool);
                    }),
                }],
                ..Default::default()
            }));
        });

        world.dependency(main_collider, tool);
    }
}