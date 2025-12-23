use winit::event::{Modifiers, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    world::{Element, Handle, World},
};

#[derive(Debug, Default)]
pub struct ModifiersTool {
    pub modifiers: Modifiers,
}

impl Element for ModifiersTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(
            world.single::<Lnwindow>().unwrap(),
            move |event: &WindowEvent, world, lnwindow| {
                if let WindowEvent::ModifiersChanged(modifiers) = event {
                    let mut tool = world.fetch_mut(this).unwrap();
                    tool.modifiers = *modifiers;
                }
            },
        );
    }
}
