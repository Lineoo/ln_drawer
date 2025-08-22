use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::interface::Interface;

#[derive(Default)]
pub struct Lnwin {
    window: Option<Lnwindow>,
}
impl ApplicationHandler for Lnwin {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let lnwindow = pollster::block_on(Lnwindow::new(event_loop));
            self.window = Some(lnwindow);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event == WindowEvent::CloseRequested {
            self.window = None;
            event_loop.exit();
            return;
        }

        if let Some(window) = &mut self.window {
            window.window_event(event);
        }
    }
}

/// The main window.
struct Lnwindow {
    window: Arc<Window>,
    interface: Interface,

    width: u32,
    height: u32,
}
impl Lnwindow {
    pub async fn new(event_loop: &ActiveEventLoop) -> Lnwindow {
        let win_attr = Window::default_attributes();

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        let interface = Interface::new(window.clone()).await;

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        Lnwindow {
            window,
            interface,
            width,
            height,
        }
    }
    pub fn window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {}
            WindowEvent::MouseInput { state, button, .. } => {}
            WindowEvent::KeyboardInput { event, .. } => {}
            WindowEvent::RedrawRequested => {
                self.interface.restructure();
                self.interface.redraw();
            }
            WindowEvent::Resized(size) => {
                self.width = size.width.max(1);
                self.height = size.height.max(1);
                self.interface.resize(size);
            }
            _ => (),
        }
    }
}
