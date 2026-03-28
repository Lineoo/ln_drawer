use hashbrown::HashMap;
use winit::event::{
    ButtonSource, ElementState, MouseButton, PointerKind, PointerSource, WindowEvent,
};

use crate::{
    lnwin::Lnwindow,
    measures::Position,
    render::camera::Camera,
    tools::collider::ToolCollider,
    world::{Element, Handle, World},
};

/// Multi touch actions that allow inputs with more points than [`PointerTool`] but no hovering
#[derive(Default)]
pub struct MultiTouchTool {
    lut: HashMap<PointerKind, (Handle<ToolCollider>, MultiTouch)>,
    blt: HashMap<Handle<ToolCollider>, Vec<PointerKind>>,
    buf: Vec<MultiTouch>,
}

impl Element for MultiTouchTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.listening_window_event(world, this);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MultiTouch {
    pub position: Position,
    pub screen: [f64; 2],
    pub status: MultiTouchStatus,
    pub data: MultiTouchData,
    pub pointer: PointerKind,
}

#[derive(Debug)]
pub struct MultiTouchGroup {
    pub active: MultiTouch,
    pub members: Vec<MultiTouch>,
}

#[derive(Debug, Clone, Copy)]
pub enum MultiTouchStatus {
    Press,
    Holding,
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct MultiTouchData {
    pub force: Option<f32>,
}

impl MultiTouchTool {
    fn listening_window_event(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, |event, world| match event {
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                position,
                button,
                ..
            } => {
                let kind = match button {
                    ButtonSource::Mouse(MouseButton::Left) => PointerKind::Mouse,
                    ButtonSource::Mouse(_) => return,
                    ButtonSource::Touch { finger_id, .. } => PointerKind::Touch(*finger_id),
                    ButtonSource::TabletTool { kind, .. } => PointerKind::TabletTool(*kind),
                    ButtonSource::Unknown(_) => PointerKind::Unknown,
                };

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let screen = lnwindow.cursor_to_screen(*position);
                let position = camera.screen_to_world_absolute(screen);
                drop((lnwindow, camera));

                let target = ToolCollider::intersect(world, position.floor())
                    .first()
                    .copied();

                let Some(target) = target else {
                    return;
                };

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();
                let touch = MultiTouch {
                    position: position.floor(),
                    screen,
                    status: MultiTouchStatus::Press,
                    data: MultiTouchData {
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
                    },
                    pointer: kind,
                };

                let replaced = tool.lut.insert(kind, (target, touch));
                let list = tool.blt.entry(target).or_default();
                list.push(kind);

                debug_assert!(replaced.is_none());

                tool.buf.reserve(list.len());
                let mut group = MultiTouchGroup {
                    active: touch,
                    members: std::mem::take(&mut tool.buf),
                };

                for member in list {
                    group.members.push(tool.lut.get(member).unwrap().1);
                }

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                group.members.clear();
                std::mem::swap(&mut tool.buf, &mut group.members);
            }

            WindowEvent::PointerMoved {
                position, source, ..
            } => {
                let kind = PointerKind::from(source.clone());

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();

                let Some((target, touch)) = tool.lut.get_mut(&kind) else {
                    return;
                };

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let screen = lnwindow.cursor_to_screen(*position);
                let position = camera.screen_to_world_absolute(screen);
                drop((lnwindow, camera));

                *touch = MultiTouch {
                    position: position.floor(),
                    screen,
                    status: MultiTouchStatus::Holding,
                    data: MultiTouchData {
                        force: match source {
                            PointerSource::Mouse => Some(1.0),
                            PointerSource::Touch { force, .. } => {
                                force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::TabletTool { data, .. } => {
                                data.force.map(|x| x.normalized(None) as f32)
                            }
                            PointerSource::Unknown => None,
                        },
                    },
                    pointer: kind,
                };

                let target = *target;
                let list = tool.blt.get_mut(&target).unwrap();

                tool.buf.reserve(list.len());
                let mut group = MultiTouchGroup {
                    active: *touch,
                    members: std::mem::take(&mut tool.buf),
                };

                for member in &*list {
                    group.members.push(tool.lut.get(member).unwrap().1);
                }

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                group.members.clear();
                std::mem::swap(&mut tool.buf, &mut group.members);
            }

            WindowEvent::PointerButton {
                state: ElementState::Released,
                position,
                button,
                ..
            } => {
                let kind = match button {
                    ButtonSource::Mouse(MouseButton::Left) => PointerKind::Mouse,
                    ButtonSource::Mouse(_) => return,
                    ButtonSource::Touch { finger_id, .. } => PointerKind::Touch(*finger_id),
                    ButtonSource::TabletTool { kind, .. } => PointerKind::TabletTool(*kind),
                    ButtonSource::Unknown(_) => PointerKind::Unknown,
                };

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let screen = lnwindow.cursor_to_screen(*position);
                let position = camera.screen_to_world_absolute(screen);
                drop((lnwindow, camera));

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();
                let Some((target, touch)) = tool.lut.get_mut(&kind) else {
                    return;
                };

                let target = *target;
                *touch = MultiTouch {
                    position: position.floor(),
                    screen,
                    status: MultiTouchStatus::Release,
                    data: MultiTouchData {
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
                    },
                    pointer: kind,
                };

                let list = tool.blt.get_mut(&target).unwrap();
                tool.buf.reserve(list.len());
                let mut group = MultiTouchGroup {
                    active: *touch,
                    members: std::mem::take(&mut tool.buf),
                };

                for member in &*list {
                    group.members.push(tool.lut.get(member).unwrap().1);
                }

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                tool.lut.remove(&kind);
                let idx = list.iter().position(|x| *x == kind).unwrap();
                list.swap_remove(idx);
                if list.is_empty() {
                    tool.blt.remove(&target);
                }

                group.members.clear();
                std::mem::swap(&mut tool.buf, &mut group.members);
            }

            _ => {}
        });
    }
}
