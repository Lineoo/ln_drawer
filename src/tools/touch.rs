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
    touches: HashMap<PointerKind, Handle<ToolCollider>>,
    groups: HashMap<Handle<ToolCollider>, Vec<MultiTouch>>,
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
    pub view: Handle<Camera>,
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
    fn listening_window_event(&mut self, world: &World, _this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, |event, world| match event {
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                position,
                button,
                ..
            } => {
                let Some(kind) = MultiTouchTool::button_to_kind(button) else {
                    return;
                };

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let Some(&(target, view)) = ToolCollider::intersect(world, screen).first() else {
                    return;
                };

                let position = world.enter(view, || {
                    let camera = world.single_fetch::<Camera>().unwrap();
                    camera.screen_to_world_absolute(screen).floor()
                });

                let touch = MultiTouch {
                    position,
                    screen,
                    view,
                    status: MultiTouchStatus::Press,
                    data: MultiTouchTool::button_to_data(button),
                    pointer: kind,
                };

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();
                let replaced = tool.touches.insert(kind, target);
                if let Some(replaced_target) = replaced {
                    // Edge-cases: duplicated TouchId is pressed
                    let list = tool.groups.get_mut(&replaced_target).unwrap();
                    let (idx, touch) = list
                        .iter_mut()
                        .enumerate()
                        .find(|(_, touch)| touch.pointer == kind)
                        .unwrap();

                    *touch = MultiTouch {
                        position,
                        screen,
                        view: touch.view,
                        status: MultiTouchStatus::Release,
                        data: MultiTouchTool::button_to_data(button),
                        pointer: kind,
                    };

                    let mut group = MultiTouchGroup {
                        active: *touch,
                        members: std::mem::take(list),
                    };

                    world.trigger(target, &group.active);
                    world.trigger(target, &group);

                    std::mem::swap(list, &mut group.members);

                    list.swap_remove(idx);
                }

                let list = tool.groups.entry(target).or_default();
                list.push(touch);

                let mut group = MultiTouchGroup {
                    active: touch,
                    members: std::mem::take(list),
                };

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                std::mem::swap(list, &mut group.members);
            }

            WindowEvent::PointerMoved {
                position, source, ..
            } => {
                let kind = PointerKind::from(source.clone());

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();
                let Some(&target) = tool.touches.get(&kind) else {
                    return;
                };

                let list = tool.groups.get_mut(&target).unwrap();
                let touch = list.iter_mut().find(|x| x.pointer == kind).unwrap();

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let position = world.enter(touch.view, || {
                    let camera = world.single_fetch::<Camera>().unwrap();
                    camera.screen_to_world_absolute(screen).floor()
                });

                *touch = MultiTouch {
                    position,
                    screen,
                    view: touch.view,
                    status: MultiTouchStatus::Holding,
                    data: MultiTouchTool::pointer_to_data(source),
                    pointer: kind,
                };

                let mut group = MultiTouchGroup {
                    active: *touch,
                    members: std::mem::take(list),
                };

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                std::mem::swap(list, &mut group.members);
            }

            WindowEvent::PointerButton {
                state: ElementState::Released,
                position,
                button,
                ..
            } => {
                let Some(kind) = MultiTouchTool::button_to_kind(button) else {
                    return;
                };

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>().unwrap();
                let Some(&target) = tool.touches.get(&kind) else {
                    return;
                };

                let list = tool.groups.get_mut(&target).unwrap();
                let (idx, touch) = list
                    .iter_mut()
                    .enumerate()
                    .find(|(_, touch)| touch.pointer == kind)
                    .unwrap();

                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let position = world.enter(touch.view, || {
                    let camera = world.single_fetch::<Camera>().unwrap();
                    camera.screen_to_world_absolute(screen).floor()
                });

                *touch = MultiTouch {
                    position,
                    screen,
                    view: touch.view,
                    status: MultiTouchStatus::Release,
                    data: MultiTouchTool::button_to_data(button),
                    pointer: kind,
                };

                let mut group = MultiTouchGroup {
                    active: *touch,
                    members: std::mem::take(list),
                };

                world.trigger(target, &group.active);
                world.trigger(target, &group);

                std::mem::swap(list, &mut group.members);

                list.swap_remove(idx);
                tool.touches.remove(&kind);
            }

            _ => {}
        });
    }

    fn button_to_kind(button: &ButtonSource) -> Option<PointerKind> {
        match button {
            ButtonSource::Mouse(MouseButton::Left) => Some(PointerKind::Mouse),
            ButtonSource::Mouse(_) => None,
            ButtonSource::Touch { finger_id, .. } => Some(PointerKind::Touch(*finger_id)),
            ButtonSource::TabletTool { kind, .. } => Some(PointerKind::TabletTool(*kind)),
            ButtonSource::Unknown(_) => Some(PointerKind::Unknown),
        }
    }

    fn button_to_data(button: &ButtonSource) -> MultiTouchData {
        MultiTouchData {
            force: match button {
                ButtonSource::Mouse(_) => Some(1.0),
                ButtonSource::Touch { force, .. } => force.map(|x| x.normalized(None) as f32),
                ButtonSource::TabletTool { data, .. } => {
                    data.force.map(|x| x.normalized(None) as f32)
                }
                ButtonSource::Unknown(_) => None,
            },
        }
    }

    fn pointer_to_data(source: &PointerSource) -> MultiTouchData {
        MultiTouchData {
            force: match source {
                PointerSource::Mouse => Some(1.0),
                PointerSource::Touch { force, .. } => force.map(|x| x.normalized(None) as f32),
                PointerSource::TabletTool { data, .. } => {
                    data.force.map(|x| x.normalized(None) as f32)
                }
                PointerSource::Unknown => None,
            },
        }
    }
}
