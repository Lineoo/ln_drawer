use std::{sync::Arc, time::Duration};

use glam::{DVec2, I64Vec2, IVec2, UVec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, ViewRef, World};
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
    measures::{FI64Ext, Rectangle},
    render::{
        Render,
        camera::{Camera, CameraDescriptor, CameraOutdated, SetCameraOutdated, UICamera},
    },
    save::{Autosave, AutosaveScheduler, Save},
    theme::Theme,
    tools::{focus::FocusTool, mouse::MouseTool, pointer::PointerTool, touch::MultiTouchTool},
    widgets::{
        WidgetRectangle,
        common::WidgetCommon,
        palette::{
            hsl::{HslRingMaterial, HslSquareMaterial},
            oklab::{OklabBarMaterial, OklabPolarMaterial},
        },
        panel::{desktop_ui::DesktopUi, mobile_ui::MobileUi},
        renderer::{
            canvas::CanvasPipeline, quad::QuadMeshPipeline, rrect::RRectMaterial,
            text::TextPipeline, vtor::VtorPipeline,
        },
    },
};

#[derive(Default)]
pub struct Lnwin {
    pub world: World,
    pub windows: HashMap<WindowId, Handle<Lnwindow>>,
}

impl ApplicationHandler for Lnwin {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.windows.is_empty() {
            let lnwindow = Lnwindow::new(event_loop);
            let window_id = lnwindow.window.id();
            let lnwindow = self.world.insert(lnwindow);
            self.windows.insert(window_id, lnwindow);
        } else {
            for &view in self.windows.values() {
                self.world.enter(view, || {
                    let mut render = self.world.single_fetch_mut::<Render>();
                    let lnwindow = self.world.single_fetch::<Lnwindow>();
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
                if let Ok(lnwindow) = self.world.try_single::<Lnwindow>() {
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
        world.scope("lnwindow");
        let here = world.here();
        world.enter(this, || world.insert(ViewRef(here)));
        world.enter_queue(this, move |world| {
            // TODO async save initialization
            Save::init(world);
            world.insert(AutosaveScheduler {
                autosave_duration: Duration::from_secs(180),
            });

            let lnwindow = world.fetch(this);
            world.insert(pollster::block_on(Render::new(&lnwindow)));
            drop(lnwindow);
            world.flush();

            Camera::init(world);

            world.flush();

            world.insert(CanvasPipeline::from_world(world));
            world.insert(QuadMeshPipeline::<HslSquareMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<HslRingMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<OklabPolarMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<OklabBarMaterial>::from_world(world));
            world.insert(QuadMeshPipeline::<RRectMaterial>::from_world(world));
            world.insert(VtorPipeline::from_world(world));
            world.insert(TextPipeline::new());
            let save = world.single_fetch::<Save>();
            world.insert(match save.sn().dark_mode {
                None | Some(true) => Theme::dark_mode(),
                Some(false) => Theme::light_mode(),
            });
            drop(save);

            world.flush();

            world.insert(PointerTool::default());
            world.insert(MouseTool::default());
            world.insert(MultiTouchTool::default());
            world.insert(FocusTool::default());

            world.flush();

            let lnwindow = world.fetch(this);
            let ui_camera = world.insert(world.build(CameraDescriptor {
                center: I64Vec2::ZERO,
                zoom: i64::q32_from_f64(lnwindow.window.scale_factor().log2()),
            }));
            world.insert(UICamera(ui_camera));
            world.observer(ui_camera, move |&SetCameraOutdated, world| {
                let mut camera = world.fetch_mut(ui_camera);
                let lnwindow = world.single_fetch::<Lnwindow>();
                let size = lnwindow.window.surface_size();
                camera.surface_size = UVec2::new(size.width, size.height);
                camera.upload();
                lnwindow.window.request_redraw();
                world.queue_trigger(ui_camera, CameraOutdated);
            });
            drop(lnwindow);
            world.observer(this, move |event: &WindowEvent, world| {
                if let WindowEvent::CloseRequested = event {
                    Autosave::autosave_all(world);
                    world.queue(|world| {
                        world.clear();
                    });
                }
            });

            world.observer(this, move |event: &WindowEvent, world| {
                if let WindowEvent::SurfaceResized(size) = event {
                    let lnwindow = world.fetch(this);
                    let mut camera = world.fetch_mut(ui_camera);

                    let scale = lnwindow.window.scale_factor();
                    world.queue_trigger(
                        lnwindow.handle(),
                        WidgetRectangle(Rectangle::new_half(
                            IVec2::ZERO,
                            (UVec2::new(size.width / 2, size.height / 2).as_dvec2() / scale)
                                .round()
                                .as_uvec2(),
                        )),
                    );

                    world.queue_trigger(camera.handle(), SetCameraOutdated);
                    camera.zoom = i64::q32_from_f64(scale.log2());
                }
            });

            world.flush();
            let save = world.single_fetch::<Save>();
            let use_desktop = std::env::var_os("LNDRAWER_DESKTOP_UI").is_some()
                || save.sn().desktop_mode.is_some_and(|x| x);
            drop(save);
            if use_desktop {
                log::warn!("using experimental desktop ui");
                world.insert(DesktopUi {
                    common: WidgetCommon {
                        camera: ui_camera,
                        visible: true,
                    },
                });
            } else {
                world.insert(MobileUi {
                    common: WidgetCommon {
                        camera: ui_camera,
                        visible: true,
                    },
                });
            }
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

    pub fn screen_to_cursor(&self, ndc: DVec2) -> PhysicalPosition<f64> {
        let size = self.window.surface_size();
        let x = (ndc.x + 1.0) * size.width as f64 / 2.0;
        let y = (1.0 - ndc.y) * size.height as f64 / 2.0;
        PhysicalPosition::new(x, y)
    }
}

#[cfg(target_os = "android")]
pub struct LnAndroid(pub AndroidApp);

#[cfg(target_os = "android")]
impl Element for LnAndroid {}
