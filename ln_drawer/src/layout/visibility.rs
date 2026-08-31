use ln_world::{Element, Handle, HandleAny, World};

use crate::widgets::{SetWidgetVisible, WidgetVisible};

pub struct VisibilityInherit {
    pub source: HandleAny,
    pub target: HandleAny,
}

impl Element for VisibilityInherit {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let ob = world.observer(self.source, move |&WidgetVisible(visible), world| {
            let this = world.fetch(this).unwrap();
            world.queue_trigger(this.target, SetWidgetVisible(visible));
        });

        world.dependency(ob, this);
        world.dependency(this, self.source);
        world.dependency(this, self.target);
    }
}
