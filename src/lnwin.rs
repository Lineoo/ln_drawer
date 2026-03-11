use std::{sync::Arc, time::Duration};

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
    measures::Size,
    render::{
        Render,
        canvas::CanvasManagerDescriptor,
        rounded::RoundedRectManagerDescriptor,
        text::TextManagerDescriptor,
        viewport::{Viewport, ViewportDescriptor},
        wireframe::WireframeManagerDescriptor,
    },
    save::{AutosaveRequest, SaveControl, SaveDatabase, SaveControlRead, AutosaveScheduler},
    stroke::StrokeLayer,
    theme::Luni,
    tools::{
        collider::ToolColliderDispatcher, focus::Focus, modifiers::ModifiersTool, mouse::MouseTool,
        pointer::PointerTool, viewport::ViewportUtils,
    },
    world::{Element, Handle, World, WorldError},
};

#[derive(Default)]
pub struct Lnwin {
    pub world: World,
}

impl ApplicationHandler for Lnwin {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.world.single::<Lnwindow>().is_err() {
            let lnwindow = Lnwindow::new(event_loop);
            self.world.insert(lnwindow);
            self.world.flush();
        } else {
            let mut render = self.world.single_fetch_mut::<Render>().unwrap();
            let lnwindow = self.world.single_fetch::<Lnwindow>().unwrap();
            render.surface_recreate(&lnwindow);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match self.world.single::<Lnwindow>() {
            Ok(window) => {
                self.world.trigger(window, &event);
                self.world.flush();
            }
            Err(_) => event_loop.exit(),
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
                world.queue(move |world| {
                    world.remove(this).unwrap();
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
            world.insert(SaveControlRead {
                name: "viewport".into(),
                read: Box::new(move |world, control| {
                    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                    let size = lnwindow.window.surface_size();

                    let control = world.fetch(control).unwrap();
                    let viewport_descriptor =
                        postcard::from_bytes::<ViewportDescriptor>(&control.read(world)).unwrap();

                    let viewport = world.build(ViewportDescriptor {
                        size: Size::new(size.width, size.height),
                        ..viewport_descriptor
                    });

                    let control = control.handle();
                    let scheduler = world.single::<AutosaveScheduler>().unwrap();
                    world.observer(scheduler, move |AutosaveRequest, world| {
                        let viewport = world.fetch(viewport).unwrap();
                        let control = world.fetch(control).unwrap();

                        let bytes = postcard::to_stdvec(&ViewportDescriptor {
                            size: viewport.size,
                            center: viewport.center,
                            zoom: viewport.zoom,
                        })
                        .unwrap();

                        control.write(world, &bytes);
                    });
                }),
            });
        });

        world.queue(|world| {
            if let Err(WorldError::SingletonNoSuch(_)) = world.single::<Viewport>() {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let size = lnwindow.window.surface_size();

                let viewport = world.build(ViewportDescriptor {
                    size: Size::new(size.width, size.height),
                    ..Default::default()
                });

                let control = SaveControl::create("viewport".into(), world, &[]);
                let scheduler = world.single::<AutosaveScheduler>().unwrap();
                world.observer(scheduler, move |AutosaveRequest, world| {
                    let viewport = world.fetch(viewport).unwrap();
                    let control = world.fetch(control).unwrap();

                    let bytes = postcard::to_stdvec(&ViewportDescriptor {
                        size: viewport.size,
                        center: viewport.center,
                        zoom: viewport.zoom,
                    })
                    .unwrap();

                    control.write(world, &bytes);
                });
            }
        });

        world.queue(|world| {
            world.build(CanvasManagerDescriptor);
            world.build(RoundedRectManagerDescriptor);
            world.build(TextManagerDescriptor);
            world.build(WireframeManagerDescriptor);
            world.insert(Luni::default());
        });

        world.queue(|world| {
            ToolColliderDispatcher::init(world);

            PointerTool::init(world);
            MouseTool::init(world);
        });

        world.queue(|world| {
            world.insert(StrokeLayer::new(world));
        });

        world.queue(|world| {
            world.insert(Focus::default());
            world.insert(ViewportUtils::default());
            world.insert(ModifiersTool::default());
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
