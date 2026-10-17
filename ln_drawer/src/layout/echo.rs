use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::widgets::{SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible};

/// Utility to build certain mysterious echo over specific element,
/// who receive all kinds of command event and emit out
/// correlated notification event immediately.
pub struct Echo<'w>(&'w World, HandleAny);

pub struct EchoWidget;

impl Echo<'_> {
    pub fn new(world: &World, this: impl HandleGeneric) -> Echo<'_> {
        Echo(world, this.untyped())
    }

    pub fn build<I: Send + 'static, O: Send + 'static>(
        &self,
        f: impl Fn(&I) -> O + Send + 'static,
    ) -> &Self {
        let &Echo(world, handle) = self;
        world.observer(handle, move |i: &I, world| {
            world.trigger(handle, &f(i));
        });
        self
    }

    pub fn widget_rectangle(&self) -> &Self {
        self.build(|&SetWidgetRectangle(val)| WidgetRectangle(val))
    }

    pub fn widget_visible(&self) -> &Self {
        self.build(|&SetWidgetVisible(val)| WidgetVisible(val))
    }
}

impl Element for EchoWidget {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        Echo::new(world, this).widget_rectangle().widget_visible();
    }
}
