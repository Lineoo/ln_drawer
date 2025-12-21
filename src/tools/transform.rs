use crate::{
    elements::menu::{MenuDescriptor, MenuEntryDescriptor},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle},
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

                        let resizing = if fetched_transform.resizable {
                            Some(new_knobs(tool, world, &mut fetched_transform))
                        } else {
                            None
                        };

                        let old = fetched_tool.active.replace(Active {
                            target: transform,
                            frame,
                            dragging: Some(Dragging {
                                element_base: fetched_transform.rect.origin,
                                pointer_base: *position,
                            }),
                            resizing,
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

                            if let Some(resizing) = &mut active.resizing {
                                update_knobs(resizing, &fetched_transform);
                            }

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

fn new_knobs(
    tool: Handle<TransformTool>,
    world: &World,
    fetched_transform: &mut Transform,
) -> Vec<ResizeKnob> {
    let mut knobs = Vec::new();

    let rect = [
        Rectangle {
            origin: fetched_transform.rect.left_down(),
            extend: Delta::new(-5, -5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.left_up(),
            extend: Delta::new(-5, 5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.right_up(),
            extend: Delta::new(5, 5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.right_down(),
            extend: Delta::new(5, -5),
        }
        .normalize(),
    ];

    let fetch = [
        |transform: &Transform| transform.rect.left_down(),
        |transform: &Transform| transform.rect.left_up(),
        |transform: &Transform| transform.rect.right_up(),
        |transform: &Transform| transform.rect.right_down(),
    ];

    let set = [
        |transform: &mut Transform, target: Position| {
            transform.rect = transform.rect.with_left_down(target);
        },
        |transform: &mut Transform, target: Position| {
            transform.rect = transform.rect.with_left_up(target);
        },
        |transform: &mut Transform, target: Position| {
            transform.rect = transform.rect.with_right_up(target);
        },
        |transform: &mut Transform, target: Position| {
            transform.rect = transform.rect.with_right_down(target);
        },
    ];

    for (i, ((rect, fetch), set)) in rect.into_iter().zip(fetch).zip(set).enumerate() {
        let wireframe = world.build(WireframeDescriptor {
            rect,
            ..Default::default()
        });

        let collider = world.insert(PointerCollider {
            rect,
            order: 55,
        });

        world.observer(collider, move |PointerHit(event), world, _| {
            let mut tool = world.fetch_mut(tool).unwrap();

            let Some(active) = &mut tool.active else {
                return;
            };

            let mut transform = world.fetch_mut(active.target).unwrap();

            let Some(resizing) = &mut active.resizing else {
                return;
            };

            let Some(knob) = resizing.get_mut(i) else {
                return;
            };

            match event {
                PointerEvent::Pressed(position) => {
                    knob.dragging.replace(Dragging {
                        element_base: fetch(&transform),
                        pointer_base: *position,
                    });
                }
                PointerEvent::Moved(position) => {
                    if let Some(dragging) = &knob.dragging {
                        let delta = *position - dragging.pointer_base;
                        set(&mut transform, dragging.element_base + delta);

                        active.frame.rect = transform.rect;
                        active.frame.upload();

                        update_knobs(resizing, &transform);
                        world.trigger(active.target, TransformUpdate);
                    }
                }
                PointerEvent::Released(_) => {
                    knob.dragging = None;
                }
            }
        });

        knobs.push(ResizeKnob {
            wireframe,
            collider,
            dragging: None,
        });
    }

    knobs
}

fn update_knobs(resizing: &mut [ResizeKnob], fetched_transform: &Transform) {
    let rect = [
        Rectangle {
            origin: fetched_transform.rect.left_down(),
            extend: Delta::new(-5, -5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.left_up(),
            extend: Delta::new(-5, 5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.right_up(),
            extend: Delta::new(5, 5),
        }
        .normalize(),
        Rectangle {
            origin: fetched_transform.rect.right_down(),
            extend: Delta::new(5, -5),
        }
        .normalize(),
    ];

    for (i, rect) in rect.into_iter().enumerate() {
        let Some(knob) = resizing.get_mut(i) else {
            return;
        };

        knob.wireframe.rect = rect;
        knob.wireframe.upload();
    }
}
