use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{Modifiers, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::{
    elements::stroke::StrokeLayer,
    measures::Size,
    render::{
        Render,
        canvas::CanvasManagerDescriptor,
        rounded::RoundedRectManagerDescriptor,
        text::TextManagerDescriptor,
        viewport::{ViewportDescriptor, ViewportManagerDescriptor},
        wireframe::WireframeManagerDescriptor,
    },
    tools::{camera::CameraTool, focus::Focus, modifiers::ModifiersTool, pointer::PointerTool},
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct Lnwin {
    world: World,
}

impl ApplicationHandler for Lnwin {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.world.single::<Lnwindow>().is_none() {
            let lnwindow = Lnwindow::new(event_loop);
            self.world.insert(lnwindow);
            self.world.flush();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match self.world.single::<Lnwindow>() {
            Some(window) => self.world.trigger(window, event),
            None => event_loop.exit(),
        }

        self.world.flush();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.world = World::default();
    }
}

/// The main window.
pub struct Lnwindow {
    window: Arc<Window>,
}

impl Element for Lnwindow {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |event: &WindowEvent, world, this| {
            if let WindowEvent::CloseRequested = event {
                world.remove(this);
            }
        });

        world.queue(move |world| {
            let lnwindow = world.fetch_mut(this).unwrap();
            world.insert(pollster::block_on(Render::new(lnwindow.window.clone())));
        });

        world.queue(|world| {
            world.insert(world.build(ViewportManagerDescriptor));
        });

        world.queue(move |world| {
            let lnwindow = world.fetch_mut(this).unwrap();
            let size = lnwindow.window.inner_size();
            world.insert(world.build(ViewportDescriptor {
                size: Size::new(size.width, size.height),
                ..Default::default()
            }));
        });

        world.queue(|world| {
            world.insert(world.build(CanvasManagerDescriptor));
            world.insert(world.build(RoundedRectManagerDescriptor));
            world.insert(world.build(TextManagerDescriptor));
            world.insert(world.build(WireframeManagerDescriptor));
        });

        world.queue(|world| {
            world.insert(Focus::default());
            world.insert(StrokeLayer::default());
            world.insert(PointerTool::default());
            world.insert(CameraTool::default());
            world.insert(ModifiersTool::default());
        });
    }
}

impl Lnwindow {
    fn new(event_loop: &ActiveEventLoop) -> Lnwindow {
        let win_attr = Window::default_attributes().with_transparent(true);

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::new(window);

        Lnwindow { window }
    }

    pub fn cursor_to_screen(&self, position: PhysicalPosition<f64>) -> [f64; 2] {
        let size = self.window.inner_size();
        let x = (position.x * 2.0) / size.width as f64 - 1.0;
        let y = 1.0 - (position.y * 2.0) / size.height as f64;
        [x, y]
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}
