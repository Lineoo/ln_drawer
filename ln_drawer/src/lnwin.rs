use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use glam::{DVec2, I64Vec2, IVec2, UVec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, ViewRef, World};
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    layout::val::{Val, ValChanged},
    measures::{FI64Ext, IRect, URect},
    render::{
        Render,
        camera::{Camera, CameraDescriptor, CameraOutdated, SetCameraOutdated, UICamera},
    },
    save::{Autosave, AutosaveScheduler, Save},
    theme::Theme,
    tools::{focus::FocusTool, mouse::MouseTool, pointer::PointerTool, touch::MultiTouchTool},
    widgets::{
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
    pub payload: Option<Arc<dyn Fn(&World, WidgetCommon) + Send + Sync>>,
}

impl ApplicationHandler for Lnwin {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.windows.is_empty() {
            let mut win_attr = WindowAttributes::default()
                .with_transparent(true)
                .with_title("LnDrawer");

            if let Some(value) = std::env::var_os("LNDRAWER_WINDOW_SIZE") {
                let value = value.to_string_lossy();
                let (width, height) = value.split_once('x').unwrap();
                let width: u32 = width.parse().unwrap();
                let height: u32 = height.parse().unwrap();
                let size = PhysicalSize::new(width, height);
                log::warn!("surface size overridden to {size:?} by LNDRAWER_WINDOW_SIZE");
                win_attr = win_attr.with_surface_size(size);
            }

            let window = event_loop.create_window(win_attr).unwrap();
            let window = Arc::from(window);

            let payload = self.payload.clone();
            let scale_factor_override = std::env::var("LNDRAWER_SCALE_FACTOR").ok().map(|value| {
                let scale = value.parse::<f64>().unwrap();
                assert!(
                    scale.is_finite() && scale > 0.0,
                    "invalid scale factor: {value}"
                );
                log::warn!("scale factor overridden to {scale} by LNDRAWER_SCALE_FACTOR");
                scale
            });
            let lnwindow = Lnwindow {
                window,
                payload,
                scale_factor_override,
            };

            let window_id = lnwindow.window.id();
            let lnwindow = self.world.insert(lnwindow);
            self.windows.insert(window_id, lnwindow);
        } else {
            for &view in self.windows.values() {
                self.world.enter(view, || {
                    let mut render = self.world.single_fetch_mut::<Render>();
                    let lnwindow = self.world.single_fetch::<Lnwindow>();
                    render.target_recreate(&lnwindow);
                });
            }
        }

        self.world.flush();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, _cause: StartCause) {
        for &view in self.windows.values() {
            self.world.enter(view, || {
                let now = Instant::now();
                let mut earliest = None;
                self.world.foreach_fetch::<Task>(|task| {
                    let earliest = earliest.get_or_insert(task.instant);
                    if task.instant < *earliest {
                        *earliest = task.instant;
                    }
                    if task.instant <= now {
                        let task = task.retrieve();
                        self.world.queue(move |world| {
                            world.remove_with(task, |task, world| {
                                log::debug!("Oi!");
                                (task.content)(world)
                            });
                        });
                    }
                });
                if let Some(earliest) = earliest {
                    log::debug!("Lets wait!");
                    event_loop.set_control_flow(ControlFlow::WaitUntil(earliest));
                } else {
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
            })
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
    pub payload: Option<Arc<dyn Fn(&World, WidgetCommon) + Send + Sync>>,
    pub scale_factor_override: Option<f64>,
}

pub struct LnwindowRect(pub Handle<Val<IRect>>);

impl Element for Lnwindow {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("lnwindow");
        world.set_autodeps(Some(this));

        let rect = world.insert(Val::new(self.window_rect()));
        world.insert(LnwindowRect(rect));

        let here = world.here();
        world.enter(this, || world.insert(ViewRef(here)));
        world.enter_queue(this, move |world| {
            world.scope("lnwindow");
            world.set_autodeps(Some(this));

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
            world.insert(TextPipeline::from_world(world));
            let save = world.single_fetch::<Save>();
            let theme = match save.sn().dark_mode {
                None | Some(true) => Theme::dark_mode(),
                Some(false) => Theme::light_mode(),
            };
            world.insert(theme.bind(world));
            world.insert(theme);
            drop(save);

            world.flush();

            world.insert(PointerTool::default());
            world.insert(MultiTouchTool::default());
            world.insert(FocusTool::default());

            world.flush();

            let lnwindow = world.fetch(this);
            let ui_camera = world.insert(world.build(CameraDescriptor {
                center: I64Vec2::ZERO,
                zoom: i64::q32_from_f64(lnwindow.scale_factor().log2()),
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
                if let WindowEvent::SurfaceResized(_) = event {
                    let lnwindow = world.fetch(this);
                    let mut rect = world.fetch_mut(rect);
                    let mut camera = world.fetch_mut(ui_camera);

                    let scale = lnwindow.scale_factor();
                    let next = lnwindow.window_rect();

                    rect.val = next;
                    world.queue_trigger(rect.handle(), ValChanged);

                    camera.zoom = i64::q32_from_f64(scale.log2());
                    world.queue_trigger(camera.handle(), SetCameraOutdated);
                }
            });

            let visible = world.insert(Val::new(true));
            let common = WidgetCommon {
                camera: ui_camera,
                visible,
            };

            world.flush();

            let this = world.fetch(this);
            if let Some(payload) = &this.payload {
                payload(world, common);
            } else {
                // TODO some should be for PageInput not MouseTool
                world.insert(MouseTool::default());

                let save = world.single_fetch::<Save>();
                let use_desktop = std::env::var_os("LNDRAWER_DESKTOP_UI").is_some()
                    || save.sn().desktop_mode.is_some_and(|x| x);
                drop(save);

                if use_desktop {
                    log::warn!("using experimental desktop ui");
                    world.insert(DesktopUi { common });
                } else {
                    world.insert(MobileUi { common });
                }
            }
        });
    }
}

impl Lnwindow {
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor_override
            .unwrap_or_else(|| self.window.scale_factor())
    }

    pub fn screen_scissor(&self, rect: IRect, camera: &Camera) -> URect {
        let size = self.window.surface_size();
        let left_up =
            self.screen_to_cursor(camera.world_to_screen(I64Vec2::q32_from_i32(rect.left_up())));
        let right_down =
            self.screen_to_cursor(camera.world_to_screen(I64Vec2::q32_from_i32(rect.right_down())));

        // ceil/floor keeps the last pixel inside fractional-scaled panels.
        let width = size.width as f64;
        let height = size.height as f64;
        let x = left_up.x.clamp(0.0, width).floor() as u32;
        let y = left_up.y.clamp(0.0, height).floor() as u32;
        let right = right_down.x.clamp(0.0, width).ceil() as u32;
        let down = right_down.y.clamp(0.0, height).ceil() as u32;

        URect {
            origin: UVec2::new(x, y),
            extend: UVec2::new(right.saturating_sub(x), down.saturating_sub(y)),
        }
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

    fn window_rect(&self) -> IRect {
        let size = self.window.surface_size();
        let scale = self.scale_factor();
        let size = DVec2::new(size.width as f64, size.height as f64);
        let half = (size / (2.0 * scale)).ceil().as_uvec2();
        IRect::new_half(IVec2::ZERO, half)
    }
}

impl Element for LnwindowRect {}

pub struct Task {
    pub instant: Instant,
    pub content: Box<dyn FnOnce(&World) + Send>,
}

impl Element for Task {}

#[cfg(target_os = "android")]
pub struct LnAndroid(pub AndroidApp);

#[cfg(target_os = "android")]
impl Element for LnAndroid {}
