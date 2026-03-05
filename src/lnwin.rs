use std::sync::Arc;

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
    elements::stroke::StrokeLayer,
    measures::{Rectangle, Size},
    render::{
        Render, canvas::CanvasManagerDescriptor, rounded::RoundedRectManagerDescriptor,
        text::TextManagerDescriptor, viewport::ViewportDescriptor,
        wireframe::WireframeManagerDescriptor,
    },
    save::Save,
    theme::Luni,
    tools::{camera::CameraTool, focus::Focus, modifiers::ModifiersTool, pointer::PointerTool},
    widgets::{WidgetClick, check_button::CheckButtonDescriptor, color_picker::ColorPicker},
    world::{Element, Handle, World},
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

        world.queue(move |world| {
            let lnwindow = world.fetch_mut(this).unwrap();
            let size = lnwindow.window.surface_size();
            world.build(ViewportDescriptor {
                size: Size::new(size.width, size.height),
                ..Default::default()
            });
        });

        world.queue(|world| {
            world.build(CanvasManagerDescriptor);
            world.build(RoundedRectManagerDescriptor);
            world.build(TextManagerDescriptor);
            world.build(WireframeManagerDescriptor);
        });

        world.queue(|world| {
            world.insert(Focus::default());
            world.insert(StrokeLayer::default());
            world.insert(PointerTool::default());
            world.insert(CameraTool::default());
            world.insert(ModifiersTool::default());

            world.insert(Luni::default());
        });

        world.queue(|world| {
            world.insert(Save::default());
        });

        world.queue(|world| {
            world.insert(ColorPicker {
                rect: Rectangle::new(0, 0, 30, 30),
                color: Default::default(),
            });

            let button = world.build(CheckButtonDescriptor {
                rect: Rectangle::new(-60, 0, -30, 30),
                checked: false,
                order: 10,
            });

            world.observer(button, move |WidgetClick, world| {
                let mut button = world.fetch_mut(button).unwrap();
                button.checked = !button.checked;
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
