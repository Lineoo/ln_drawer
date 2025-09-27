use crate::{
    elements::Element,
    world::{ElementHandle, WorldCell},
};

pub struct Focus {
    on: Option<ElementHandle>,
}
impl Element for Focus {}
impl Focus {
    pub fn get(&self) -> Option<ElementHandle> {
        self.on
    }

    pub fn set(&mut self, on: Option<ElementHandle>) {
        self.on = on
    }
}

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
