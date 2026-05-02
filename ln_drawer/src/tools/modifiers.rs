use ln_world::{Element, Handle, World};
use winit::event::{Modifiers, WindowEvent};

use crate::lnwin::Lnwindow;

#[derive(Debug, Default)]
pub struct ModifiersTool {
    pub modifiers: Modifiers,
}

impl Element for ModifiersTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();

        world.observer(lnwindow, move |event: &WindowEvent, world| {
            if let WindowEvent::ModifiersChanged(modifiers) = event {
                let mut tool = world.fetch_mut(this).unwrap();
                tool.modifiers = *modifiers;
            }
        });
    }
}
