use winit::event::{KeyEvent, WindowEvent};

use crate::world::{Element, ElementHandle, WorldCell, WorldCellEntry};

#[derive(Default)]
pub struct Focus {
    on: Option<ElementHandle>,
}
impl Element for Focus {
    fn when_inserted(&mut self, entry: WorldCellEntry) {
        entry.world().observe::<WindowEvent>(|event, world| {
            if let WindowEvent::KeyboardInput { event, .. } = event
                && let Some(focus) = world.single::<Focus>()
                && let Some(focus_on) = focus.get()
                && let Some(mut focus_on) = world.entry(focus_on)
            {
                focus_on.trigger(FocusInput(event.clone()));
            }
        });
    }
}
impl Focus {
    pub fn get(&self) -> Option<ElementHandle> {
        self.on
    }

    pub fn set(&mut self, on: Option<ElementHandle>, world: &WorldCell) {
        let off = self.on;
        self.on = on;
        if off != on {
            if let Some(mut off) = off.and_then(|off| world.entry(off)) {
                off.trigger(FocusOff);
            }
            if let Some(mut on) = on.and_then(|on| world.entry(on)) {
                on.trigger(FocusOn);
            }
        }
    }
}

pub struct FocusOn;
pub struct FocusOff;

pub struct FocusInput(pub KeyEvent);

pub trait Focusable: Element {
    fn is_focusable(&self) -> bool {
        true
    }
}

pub trait FocusableExt: Focusable + Sized {
    fn register_focus(&mut self, handle: ElementHandle, world: &WorldCell);
}
impl<T: Focusable> FocusableExt for T {
    fn register_focus(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.register::<dyn Focusable>(|this| this.downcast_ref::<Self>().unwrap());
        this.register_mut::<dyn Focusable>(|this| this.downcast_mut::<Self>().unwrap());
    }
}
