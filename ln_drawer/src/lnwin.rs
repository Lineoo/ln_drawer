use std::{sync::Arc, time::Duration};

use hashbrown::HashMap;
use ln_world::{Element, Handle, ViewOptions, World};
use palette::Srgba;
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    layout::{
        luni::{LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::{Position, Rectangle, Size},
    render::{
        Render,
        camera::{Camera, CameraUtils, MainCamera},
        canvas::CanvasManagerDescriptor,
        rectangle::RectangleMesh,
        rounded::RoundedRect,
        text::TextManagerDescriptor,
    },
    save::{Autosave, AutosaveScheduler, SaveDatabase},
    stroke::{StrokeLayer, modifier::Modifier},
    theme::ColorScheme,
    tools::{
        collider::ToolColliderDispatcher, focus::Focus, modifiers::ModifiersTool, mouse::MouseTool,
        pointer::PointerTool, touch::MultiTouchTool,
    },
    widgets::{
        WidgetClick, WidgetRectangle,
        button::{Button, ButtonChecked, ButtonImage},
        palette::{ColorPicker, hsl::PaletteHslMaterial},
    },
};

#[derive(Default)]
pub struct Lnwin {
    pub world: World,
    pub windows: HashMap<WindowId, Handle>,
}

impl ApplicationHandler for Lnwin {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.windows.is_empty() {
            let lnwindow = Lnwindow::new(event_loop);
            let root = self.world.here();
            let window_id = lnwindow.window.id();
            let lnwindow = self.world.insert(lnwindow);
            self.windows.insert(window_id, lnwindow.untyped());
            self.world.enter(lnwindow, || {
                self.world.option(ViewOptions { refs: vec![root] });
            });
        } else {
            for &view in self.windows.values() {
                self.world.enter(view, || {
                    let mut render = self.world.single_fetch_mut::<Render>().unwrap();
                    let lnwindow = self.world.single_fetch::<Lnwindow>().unwrap();
                    render.surface_recreate(&lnwindow);
                });
            }
        }

        self.world.flush();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(&view) = self.windows.get(&window_id) {
            self.world.enter(view, || {
                if let Ok(lnwindow) = self.world.single::<Lnwindow>() {
                    self.world.trigger(lnwindow, &event);
                } else {
                    self.windows.remove(&window_id);
                }
            });

            self.world.flush();
        }

        if self.windows.is_empty() {
            event_loop.exit()
        }
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for &view in self.windows.values() {
            self.world.enter(view, || {
                Autosave::autosave_all(&self.world);
            });
        }
    }
}

/// The main window.
pub struct Lnwindow {
    pub window: Arc<dyn Window>,
}

impl Element for Lnwindow {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |event: &WindowEvent, world| {
            if let WindowEvent::CloseRequested = event {
                Autosave::autosave_all(world);
                world.queue(|world| {
                    world.clear();
                });
            }
        });

        world.queue(move |world| {
            let lnwindow = world.fetch_mut(this).unwrap();
            world.insert(pollster::block_on(Render::new(&lnwindow)));
        });

        world.queue(|world| {
            SaveDatabase::init(world);
            world.insert(AutosaveScheduler {
                autosave_duration: Duration::from_secs(10),
            });
        });

        world.queue(|world| {
            Camera::init(world);
            world.flush();

            world.build(CanvasManagerDescriptor);
            world.build(TextManagerDescriptor);
            RoundedRect::init(world);
            RectangleMesh::<PaletteHslMaterial>::init(world);
            world.insert(ColorScheme::default());
        });

        world.queue(|world| {
            world.insert(ToolColliderDispatcher);
            world.insert(PointerTool::default());
            world.insert(MouseTool::default());
            world.insert(MultiTouchTool::default());
            world.insert(Focus::default());
            world.insert(ModifiersTool::default());
        });

        world.queue(|world| {
            let here = world.here();

            let camera1 = Camera::build_from_save(world, "camera1");
            world.insert(MainCamera(camera1));
            world.enter(camera1, || {
                world.option(ViewOptions { refs: vec![here] });
                world.queue(|world| {
                    world.insert(StrokeLayer::new(world));
                    world.insert(CameraUtils::default());
                });
            });

            world.flush();

            let camera2 = Camera::build_from_save(world, "camera2");
            world.enter(camera2, || {
                let stroke = world.enter(camera1, || world.single::<StrokeLayer>().unwrap());
                world.option(ViewOptions {
                    refs: vec![here, stroke.untyped()],
                });
                world.queue(|world| {
                    world.insert(ColorPicker::default());
                    world.insert(CameraUtils::default());
                });

                world.queue(|world| {
                    let lnwindow = world.single::<Lnwindow>().unwrap();
                    world.observer(lnwindow, move |event: &WindowEvent, world| {
                        if let WindowEvent::SurfaceResized(size) = event {
                            world.trigger(
                                lnwindow,
                                &WidgetRectangle(Rectangle::new_half(
                                    Position::ZERO,
                                    Size::new(size.width / 2, size.height / 2),
                                )),
                            );
                        }
                    });

                    // luni test

                    let parent = world.insert(Button {
                        rect: Rectangle::new(0, 0, 100, 100),
                        order: 0,
                        color: Srgba::new(0.863, 0.863, 0.863, 1.0),
                        active_color: Srgba::new(0.863, 0.863, 0.863, 1.0),
                        press_color: Srgba::new(0.863, 0.863, 0.863, 1.0),
                        ..Default::default()
                    });

                    let child0 = world.insert(Button {
                        rect: Rectangle::new(0, 0, 100, 100),
                        order: 10,
                        color: Srgba::new(0.5, 0.5, 0.5, 0.0),
                        active_color: Srgba::new(0.5, 0.5, 0.5, 0.2),
                        press_color: Srgba::new(0.5, 0.5, 0.5, 0.3),
                        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
                        image: Some(ButtonImage {
                            transform: TransformValue::anchor(
                                (0.5, 0.5),
                                Rectangle::new_half(Position::ZERO, Size::splat(12)),
                                Position::ZERO,
                            ),
                            bytes: include_bytes!("../res/interface/brush.png"),
                        }),
                        ..Default::default()
                    });

                    let child1 = world.insert(Button {
                        rect: Rectangle::new(0, 0, 100, 100),
                        order: 10,
                        color: Srgba::new(0.5, 0.5, 0.5, 0.0),
                        active_color: Srgba::new(0.5, 0.5, 0.5, 0.2),
                        press_color: Srgba::new(0.5, 0.5, 0.5, 0.3),
                        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
                        image: Some(ButtonImage {
                            transform: TransformValue::anchor(
                                (0.5, 0.5),
                                Rectangle::new_half(Position::ZERO, Size::splat(12)),
                                Position::ZERO,
                            ),
                            bytes: include_bytes!("../res/interface/pen.png"),
                        }),
                        ..Default::default()
                    });

                    world.insert(Transform {
                        value: TransformValue {
                            left: TransformEdge {
                                anchor: 1.0,
                                offset: -120,
                            },
                            down: TransformEdge {
                                anchor: 0.5,
                                offset: 200,
                            },
                            right: TransformEdge {
                                anchor: 1.0,
                                offset: -50,
                            },
                            up: TransformEdge {
                                anchor: 0.5,
                                offset: -200,
                            },
                        },
                        source: lnwindow.untyped(),
                        target: parent.untyped(),
                    });

                    world.insert(LuniFlex {
                        parent: (
                            parent.untyped(),
                            LuniParent {
                                axis: LuniAxis::Column,
                                template: LuniChildTemplate {
                                    basis: 10,
                                    margin: LuniRect {
                                        left: 4,
                                        bottom: 4,
                                        right: 4,
                                        top: 4,
                                    },
                                    ..Default::default()
                                },
                                padding: LuniRect {
                                    left: 4,
                                    bottom: 4,
                                    right: 4,
                                    top: 4,
                                },
                                ..Default::default()
                            },
                        ),
                        children: vec![
                            (
                                child0.untyped(),
                                LuniChild {
                                    basis: Some(54),
                                    shrink: Some(1.0),
                                    ..Default::default()
                                },
                            ),
                            (
                                child1.untyped(),
                                LuniChild {
                                    basis: Some(54),
                                    shrink: Some(1.0),
                                    ..Default::default()
                                },
                            ),
                        ],
                    });

                    world.observer(child0, move |&WidgetClick, world| {
                        world.trigger(child0, &ButtonChecked(true));
                        world.trigger(child1, &ButtonChecked(false));
                        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
                        stroke.modifier = Modifier {
                            min_size: 0.0,
                            max_size: 6.0,
                            size_force_exp: 1.0,
                            min_flow: 0.7,
                            max_flow: 1.0,
                            flow_force_exp: 2.0,
                            softness: 0.2,
                            ..stroke.modifier
                        };
                    });

                    world.observer(child1, move |&WidgetClick, world| {
                        world.trigger(child0, &ButtonChecked(false));
                        world.trigger(child1, &ButtonChecked(true));
                        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
                        stroke.modifier = Modifier {
                            min_size: 1.0,
                            max_size: 25.0,
                            size_force_exp: 1.0,
                            min_flow: 0.1,
                            max_flow: 1.0,
                            flow_force_exp: 1.0,
                            softness: 0.5,
                            ..stroke.modifier
                        };
                    });

                    world.queue_trigger(parent, WidgetRectangle(Rectangle::new(0, 0, 500, 100)));
                });
            });
        });
    }
}

impl Lnwindow {
    fn new(event_loop: &dyn ActiveEventLoop) -> Lnwindow {
        let win_attr = WindowAttributes::default()
            .with_transparent(true)
            .with_title("LnDrawer");

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::from(window);

        Lnwindow { window }
    }

    pub fn cursor_to_screen(&self, position: PhysicalPosition<f64>) -> [f64; 2] {
        let size = self.window.surface_size();
        let x = (position.x * 2.0) / size.width as f64 - 1.0;
        let y = 1.0 - (position.y * 2.0) / size.height as f64;
        [x, y]
    }
}

#[cfg(target_os = "android")]
pub struct LnAndroid(pub AndroidApp);

#[cfg(target_os = "android")]
impl Element for LnAndroid {}
