use glam::{DVec2, I64Vec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, HandleAny, World};
use winit::event::{
    ButtonSource, ElementState, MouseButton, PointerKind, PointerSource, WindowEvent,
};

use crate::{lnwin::Lnwindow, render::camera::Camera, tools::collider::ToolControl};

/// Multi touch actions that allow inputs with more points than [`PointerTool`] but no hovering
#[derive(Default)]
pub struct MultiTouchTool {
    touches: HashMap<PointerKind, Touch>,
    groups: HashMap<HandleAny, Vec<MultiTouch>>,
}

impl Element for MultiTouchTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.listening_window_event(world, this);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MultiTouch {
    pub position: I64Vec2,
    pub status: MultiTouchStatus,
    pub data: MultiTouchData,
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
    pub screen: DVec2,
    pub kind: PointerKind,
    pub force: Option<f32>,
}

struct Touch {
    camera: Handle<Camera>,
    handle: HandleAny,
}

impl MultiTouchTool {
    fn listening_window_event(&mut self, world: &World, _this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();
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

                let lnwindow = world.single_fetch::<Lnwindow>();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let Some(hit) = ToolControl::intersect_main(world, screen) else {
                    return;
                };

                let camera = world.fetch(hit.camera);
                let position = camera.screen_to_world(screen);

                let event = MultiTouch {
                    position,
                    status: MultiTouchStatus::Press,
                    data: MultiTouchData {
                        screen,
                        kind,
                        force: MultiTouchTool::button_to_force(button),
                    },
                };

                let touch = Touch {
                    camera: hit.camera,
                    handle: hit.handle,
                };

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>();
                let replaced = tool.touches.insert(kind, touch);
                if let Some(replaced) = replaced {
                    // Edge-cases: duplicated TouchId is pressed
                    let list = tool.groups.get_mut(&replaced.handle).unwrap();
                    let (idx, event) = list
                        .iter_mut()
                        .enumerate()
                        .find(|(_, touch)| touch.data.kind == kind)
                        .unwrap();

                    *event = MultiTouch {
                        position,
                        status: MultiTouchStatus::Release,
                        data: MultiTouchData {
                            screen,
                            kind,
                            force: MultiTouchTool::button_to_force(button),
                        },
                    };

                    let mut group = MultiTouchGroup {
                        active: *event,
                        members: std::mem::take(list),
                    };

                    world.trigger(replaced.handle, &group.active);
                    world.trigger(replaced.handle, &group);

                    std::mem::swap(list, &mut group.members);

                    list.swap_remove(idx);
                }

                let list = tool.groups.entry(hit.handle).or_default();
                list.push(event);

                let mut group = MultiTouchGroup {
                    active: event,
                    members: std::mem::take(list),
                };

                world.trigger(hit.handle, &group.active);
                world.trigger(hit.handle, &group);

                std::mem::swap(list, &mut group.members);
            }

            WindowEvent::PointerMoved {
                position, source, ..
            } => {
                let kind = PointerKind::from(source.clone());

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>();
                let Some(target) = tool.touches.get(&kind) else {
                    return;
                };

                let list = tool.groups.get_mut(&target.handle).unwrap();
                let touch = list.iter_mut().find(|x| x.data.kind == kind).unwrap();

                let lnwindow = world.single_fetch::<Lnwindow>();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let camera = world.fetch(target.camera);
                let position = camera.screen_to_world(screen);
                drop(camera);

                *touch = MultiTouch {
                    position,
                    status: MultiTouchStatus::Holding,
                    data: MultiTouchData {
                        screen,
                        kind,
                        force: MultiTouchTool::pointer_to_force(source),
                    },
                };

                let mut group = MultiTouchGroup {
                    active: *touch,
                    members: std::mem::take(list),
                };

                world.trigger(target.handle, &group.active);
                world.trigger(target.handle, &group);

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

                let tool = &mut *world.single_fetch_mut::<MultiTouchTool>();
                let Some(touch) = tool.touches.remove(&kind) else {
                    return;
                };

                let lnwindow = world.single_fetch::<Lnwindow>();
                let screen = lnwindow.cursor_to_screen(*position);
                drop(lnwindow);

                let camera = world.fetch(touch.camera);
                let position = camera.screen_to_world(screen);
                drop(camera);

                let active = MultiTouch {
                    position,
                    status: MultiTouchStatus::Release,
                    data: MultiTouchData {
                        screen,
                        kind,
                        force: MultiTouchTool::button_to_force(button),
                    },
                };

                let list = tool.groups.get_mut(&touch.handle).unwrap();
                list.retain(|touch| touch.data.kind != kind);

                let mut group = MultiTouchGroup {
                    active,
                    members: std::mem::take(list),
                };

                world.trigger(touch.handle, &group.active);
                world.trigger(touch.handle, &group);

                std::mem::swap(list, &mut group.members);
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

    fn button_to_force(button: &ButtonSource) -> Option<f32> {
        match button {
            ButtonSource::Mouse(_) => Some(1.0),
            ButtonSource::Touch { force, .. } => force.map(|x| x.normalized(None) as f32),
            ButtonSource::TabletTool { data, .. } => data.force.map(|x| x.normalized(None) as f32),
            ButtonSource::Unknown(_) => None,
        }
    }

    fn pointer_to_force(source: &PointerSource) -> Option<f32> {
        match source {
            PointerSource::Mouse => Some(1.0),
            PointerSource::Touch { force, .. } => force.map(|x| x.normalized(None) as f32),
            PointerSource::TabletTool { data, .. } => data.force.map(|x| x.normalized(None) as f32),
            PointerSource::Unknown => None,
        }
    }
}
