use std::{sync::Arc, time::Duration};

use glam::{DVec2, IVec2, UVec2};
use hashbrown::HashMap;
use ln_world::{ElemRef, Element, Handle, ViewRef, World};
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
    layer::{input::LayerInput, wrapper::LayerWrapper},
    measures::{FI64Ext, Rectangle},
    render::{
        Render,
        camera::{Camera, CameraDescriptor, CameraUtils, MainCamera, UICamera},
        rounded::RoundedRect,
    },
    save::{Autosave, AutosaveScheduler, SaveDatabase},
    theme::Theme,
    tools::{
        collider::ToolColliderDispatcher, focus::FocusTool, modifiers::ModifiersTool,
        mouse::MouseTool, pointer::PointerTool, touch::MultiTouchTool,
    },
    widgets::{
        WidgetRectangle,
        palette::{
            hsl::HslPanelMaterial,
            oklab::{OklabBarMaterial, OklabPolarMaterial},
        },
        panel::side_docker::side_docker,
        renderer::{
            canvas::CanvasPipeline, quad::QuadMeshPipeline, rrect::RRectMaterial,
            text::TextPipeline,
        },
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
                self.world.insert(ViewRef(root));
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
                autosave_duration: Duration::from_secs(180),
            });
        });

        world.queue(|world| {
            Camera::init(world);
            world.flush();

            world.insert(CanvasPipeline::from_world(world));
            world.insert(QuadMeshPipeline::<HslPanelMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<OklabPolarMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<OklabBarMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<RRectMaterial>::from_world(world));
            world.insert(TextPipeline::new());
            RoundedRect::init(world);
            world.insert(Theme::default());
        });

        world.queue(|world| {
            world.insert(ToolColliderDispatcher);
            world.insert(PointerTool::default());
            world.insert(MouseTool::default());
            world.insert(MultiTouchTool::default());
            world.insert(FocusTool::default());
            world.insert(ModifiersTool::default());
        });

        world.queue(|world| {
            let here = world.here();
            let camera1 = Camera::build_from_save(world, "camera1");
            world.insert(MainCamera(camera1));

            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            let size = lnwindow.window.surface_size();
            let camera2 = world.build(CameraDescriptor {
                size: UVec2::new(size.width, size.height),
                zoom: i64::q32_from_f64(lnwindow.window.scale_factor().log2()),
                ..Default::default()
            });
            world.insert(UICamera(camera2));
            drop(lnwindow);

            world.flush();

            world.enter(camera1, || {
                world.insert(ViewRef(here));
            });
            world.enter(camera2, || {
                world.insert(ViewRef(here));
            });

            world.flush();
            world.enter(camera1, || {
                world.queue(|world| {
                    world.insert(LayerWrapper::new(world));
                    world.insert(LayerInput::default());
                    let camera = world.single_fetch::<Camera>().unwrap();
                    world.insert(CameraUtils::new(&camera));
                    let lnwindow = world.single::<Lnwindow>().unwrap();
                    world.observer(lnwindow, move |event: &WindowEvent, world| {
                        if let WindowEvent::SurfaceResized(size) = event {
                            let mut camera = world.single_fetch_mut::<CameraUtils>().unwrap();
                            camera.camera_size(UVec2::new(size.width, size.height));
                        }
                    });
                });
            });

            world.flush();
            world.enter(camera2, || {
                let stroke = world.enter(camera1, || world.single::<LayerWrapper>().unwrap());
                let input = world.enter(camera1, || world.single::<LayerInput>().unwrap());
                world.insert(ElemRef(stroke.untyped()));
                world.insert(ElemRef(input.untyped()));
                world.queue(move |world| {
                    let camera = world.single_fetch::<Camera>().unwrap();
                    world.insert(CameraUtils::new(&camera));
                    let lnwindow = world.single::<Lnwindow>().unwrap();
                    world.observer(lnwindow, move |event: &WindowEvent, world| {
                        if let WindowEvent::SurfaceResized(size) = event {
                            let lnwindow = world.fetch(lnwindow).unwrap();
                            let mut camera2 = world.fetch_mut(camera2).unwrap();
                            let mut camera = world.single_fetch_mut::<CameraUtils>().unwrap();

                            let scale = lnwindow.window.scale_factor();
                            world.queue_trigger(
                                lnwindow.handle(),
                                WidgetRectangle(Rectangle::new_half(
                                    IVec2::ZERO,
                                    (UVec2::new(size.width / 2, size.height / 2).as_dvec2()
                                        / scale)
                                        .round()
                                        .as_uvec2(),
                                )),
                            );

                            camera2.zoom = i64::q32_from_f64(scale.log2());
                            camera.update_from(&camera2);
                        }
                    });
                });

                world.queue(|world| side_docker(world));
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

    pub fn cursor_to_screen(&self, position: PhysicalPosition<f64>) -> DVec2 {
        let size = self.window.surface_size();
        let x = (position.x * 2.0) / size.width as f64 - 1.0;
        let y = 1.0 - (position.y * 2.0) / size.height as f64;
        DVec2::new(x, y)
    }
}

#[cfg(target_os = "android")]
pub struct LnAndroid(pub AndroidApp);

#[cfg(target_os = "android")]
impl Element for LnAndroid {}
