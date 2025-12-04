use winit::event::{KeyEvent, WindowEvent};

use crate::world::{Element, Handle, World};

#[derive(Default)]
pub struct Focus {
    on: Option<Handle>,
}

impl Element for Focus {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |event: &WindowEvent, world, _| {
            if let Some(focus) = world.single_fetch::<Focus>()
                && let Some(focus_on) = focus.get()
                && let WindowEvent::KeyboardInput { event, .. } = event
            {
                world.trigger(focus_on, FocusInput(event.clone()));
            }
        });
    }
}

impl Focus {
    pub fn get(&self) -> Option<Handle> {
        self.on
    }

    pub fn set(&mut self, on: Option<Handle>, world: &World) {
        let off = self.on;
        self.on = on;
        if off != on {
            if let Some(off) = off {
                world.trigger(off, &FocusOff);
            }
            if let Some(on) = on {
                world.trigger(on, &FocusOn);
            }
        }
    }
}

pub struct FocusOn;
pub struct FocusOff;

pub struct FocusInput(pub KeyEvent);
