use std::{sync::Arc, time::Duration};

use hashbrown::HashMap;
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
    render::{
        Render,
        camera::{Camera, CameraUtils, CameraVisits},
        canvas::CanvasManagerDescriptor,
        rectangle::RectangleMesh,
        rounded::RoundedRect,
        text::TextManagerDescriptor,
        wireframe::WireframeManagerDescriptor,
    },
    save::{Autosave, AutosaveScheduler, SaveControl},
    stroke::StrokeLayer,
    theme::Luni,
    tools::{
        collider::ToolColliderDispatcher, focus::Focus, modifiers::ModifiersTool, mouse::MouseTool,
        pointer::PointerTool, touch::MultiTouchTool,
    },
    widgets::palette::{ColorPicker, hsl::PaletteHslMaterial},
    world::{Element, Handle, ViewOptions, World},
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
            SaveControl::init_database(world);
            world.insert(AutosaveScheduler {
                autosave_duration: Duration::from_secs(10),
            });
        });

        world.queue(|world| {
            Camera::init(world);
            world.flush();

            world.build(CanvasManagerDescriptor);
            world.build(TextManagerDescriptor);
            world.build(WireframeManagerDescriptor);
            RoundedRect::init(world);
            RectangleMesh::<PaletteHslMaterial>::init(world);
            world.insert(Luni::default());
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
            let layer1 = world.insert(());
            let layer2 = world.insert(());
            let here = world.here();

            world.enter(layer1, || {
                world.option(ViewOptions { refs: vec![here] });

                world.queue(|world| {
                    Camera::singleton(world, "camera1");
                    world.flush();
                    world.insert(StrokeLayer::new(world));
                    world.insert(CameraUtils::default());
                    world.insert(ColorPicker);
                });
            });

            world.enter(layer2, || {
                world.option(ViewOptions { refs: vec![here] });

                world.queue(|world| {
                    Camera::singleton(world, "camera2");
                    world.flush();
                    world.insert(CameraUtils::default());
                });
            });

            world.insert(CameraVisits {
                views: vec![layer1.untyped(), layer2.untyped()],
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
impl Element for AndroidApp {}
