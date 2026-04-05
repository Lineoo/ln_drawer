use winit::event::{
    ButtonSource, ElementState, MouseButton, PointerKind, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::Position,
    render::camera::Camera,
    tools::collider::{ToolCollider, ToolColliderChanged, ToolColliderDispatcher},
    world::{Element, Handle, World},
};

/// Guaranteed for single-pointer operations like mouse cursor or the first-touch finger.
#[derive(Default)]
pub struct PointerTool {
    /// the main pointer that takes effect
    pointer: Option<Pointer>,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHit {
    pub position: Position,
    pub screen: [f64; 2],
    pub status: PointerHitStatus,
    pub data: PointerHitData,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHover {
    pub position: Position,
    pub screen: [f64; 2],
    pub status: PointerHoverStatus,
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHitStatus {
    Press,
    Moving,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerHoverStatus {
    Enter,
    Moving,
    Leave,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerData {
    pub position: Position,
    pub screen: [f64; 2],
    pub pointer: PointerKind,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerHitData {
    pub force: Option<f32>,
}

struct Pointer {
    screen: [f64; 2],
    kind: PointerKind,
    hovering: Option<Hover>,
    pressed: Option<Press>,
}

#[derive(Clone, Copy)]
struct Hover {
    position: Position,
    view: Handle,
    handle: Handle<ToolCollider>,
}

#[derive(Clone, Copy)]
struct Press {
    force: Option<f32>,
}

impl PointerTool {
    fn alloc_pointer(&mut self, kind: PointerKind) -> Option<&mut Pointer> {
        if self.pointer.is_none() {
            self.pointer = Some(Pointer {
                screen: Default::default(),
                kind,
                hovering: None,
                pressed: None,
            });

            self.pointer.as_mut()
        } else if let Some(pointer) = &self.pointer
            && pointer.kind == kind
        {
            self.pointer.as_mut()
        } else {
            None
        }
    }
}

impl Element for PointerTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();

            match event {
                WindowEvent::PointerMoved {
                    position, source, ..
                } => {
                    let kind = PointerKind::from(source.clone());

                    let Some(pointer) = this.alloc_pointer(kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    drop(lnwindow);

                    if let Some(press) = &mut pointer.pressed {
                        press.force = match source {
                            PointerSource::Mouse => Some(1.0),
                            PointerSource::Touch { force, .. } => {
                                force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::TabletTool { data, .. } => {
                                data.force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::Unknown => None,
                        };
                    }
                    pointer.update_position(world, screen);
                }

                WindowEvent::PointerButton {
                    position,
                    button,
                    state,
                    ..
                } => {
                    let kind = match button {
                        ButtonSource::Mouse(MouseButton::Left) => PointerKind::Mouse,
                        ButtonSource::Mouse(_) => return,
                        ButtonSource::Touch { finger_id, .. } => PointerKind::Touch(*finger_id),
                        ButtonSource::TabletTool { kind, .. } => PointerKind::TabletTool(*kind),
                        ButtonSource::Unknown(_) => PointerKind::Unknown,
                    };

                    let Some(pointer) = this.alloc_pointer(kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    drop(lnwindow);

                    pointer.update_position(world, screen);
                    pointer.update_pressed(
                        world,
                        match state {
                            ElementState::Pressed => Some(Press {
                                force: match button {
                                    ButtonSource::Mouse(_) => Some(1.0),
                                    ButtonSource::Touch { force, .. } => {
                                        force.map(|x| x.normalized(None) as f32)
                                    }
                                    ButtonSource::TabletTool { data, .. } => {
                                        data.force.map(|x| x.normalized(None) as f32)
                                    }
                                    ButtonSource::Unknown(_) => None,
                                },
                            }),
                            ElementState::Released => None,
                        },
                    );
                }

                WindowEvent::PointerEntered { position, kind, .. } => {
                    let Some(pointer) = this.alloc_pointer(*kind) else {
                        return;
                    };

                    let screen = lnwindow.cursor_to_screen(*position);
                    drop(lnwindow);

                    pointer.update_position(world, screen);
                }

                WindowEvent::PointerLeft { position, kind, .. } => {
                    let Some(pointer) = this.alloc_pointer(*kind) else {
                        return;
                    };

                    if let Some(position) = *position {
                        let screen = lnwindow.cursor_to_screen(position);
                        drop(lnwindow);

                        pointer.update_position(world, screen);
                    } else {
                        drop(lnwindow);
                    }

                    pointer.update_hovering(world, None);
                    this.pointer = None;
                }

                _ => {}
            }
        });

        let dispatcher = world.single::<ToolColliderDispatcher>().unwrap();
        let ob = world.observer(dispatcher, |&ToolColliderChanged(collider), world| {
            let mut tool = world.single_fetch_mut::<PointerTool>().unwrap();
            if let Some(pointer) = &mut tool.pointer
                && pointer.hovering.is_some_and(|x| x.handle == collider)
            {
                pointer.recalculate_hovering(world);
            }
        });

        world.dependency(ob, this);
        world.dependency(this, dispatcher);
    }
}

impl Pointer {
    fn update_position(&mut self, world: &World, screen: [f64; 2]) {
        self.screen = screen;

        self.recalculate_hovering(world);

        if let Some(hovering) = self.hovering {
            if let Some(pressed) = self.pressed {
                world.enter(hovering.view, || {
                    world.queue_trigger(
                        hovering.handle,
                        PointerHit {
                            position: hovering.position,
                            screen,
                            status: PointerHitStatus::Moving,
                            data: PointerHitData {
                                force: pressed.force,
                            },
                            pointer: self.kind,
                        },
                    );
                });
            }

            world.enter(hovering.view, || {
                world.queue_trigger(
                    hovering.handle,
                    PointerHover {
                        position: hovering.position,
                        screen,
                        status: PointerHoverStatus::Moving,
                        pointer: self.kind,
                    },
                );
            });
        }
    }

    fn update_pressed(&mut self, world: &World, pressed: Option<Press>) {
        if let Some(hovering) = self.hovering {
            let hit = match (self.pressed, pressed) {
                (None, Some(press)) => Some(PointerHit {
                    position: hovering.position,
                    screen: self.screen,
                    status: PointerHitStatus::Press,
                    data: PointerHitData { force: press.force },
                    pointer: self.kind,
                }),
                (Some(press), None) => Some(PointerHit {
                    position: hovering.position,
                    screen: self.screen,
                    status: PointerHitStatus::Release,
                    data: PointerHitData { force: press.force },
                    pointer: self.kind,
                }),
                _ => None,
            };

            if let Some(hit) = hit {
                world.enter(hovering.view, || {
                    world.queue_trigger(hovering.handle, hit);
                });
            }
        }

        self.pressed = pressed;
        self.recalculate_hovering(world);
    }

    fn update_hovering(&mut self, world: &World, hovering: Option<Hover>) {
        let previous = self.hovering;
        self.hovering = hovering;

        if hovering.map(|x| x.handle) == previous.map(|x| x.handle) {
            return;
        }

        if let Some(previous) = previous {
            world.enter(previous.view, || {
                world.queue_trigger(
                    previous.handle,
                    PointerHover {
                        position: previous.position,
                        screen: self.screen,
                        status: PointerHoverStatus::Leave,
                        pointer: self.kind,
                    },
                );
            });
        }

        if let Some(hovering) = hovering {
            world.enter(hovering.view, || {
                world.queue_trigger(
                    hovering.handle,
                    PointerHover {
                        position: hovering.position,
                        screen: self.screen,
                        status: PointerHoverStatus::Enter,
                        pointer: self.kind,
                    },
                );
            });
        }
    }

    fn recalculate_hovering(&mut self, world: &World) {
        if self.pressed.is_some() {
            let hovering = self.hovering.unwrap();
            let position = world.enter(hovering.view, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                camera.screen_to_world_absolute(self.screen).floor()
            });

            self.update_hovering(
                world,
                Some(Hover {
                    position,
                    view: hovering.view,
                    handle: hovering.handle,
                }),
            );
        } else if let Some(&(each, view)) = ToolCollider::intersect(world, self.screen).first() {
            let position = world.enter(view, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                camera.screen_to_world_absolute(self.screen).floor()
            });

            self.update_hovering(
                world,
                Some(Hover {
                    position,
                    view,
                    handle: each,
                }),
            );
        } else {
            self.update_hovering(world, None);
        }
    }
}
