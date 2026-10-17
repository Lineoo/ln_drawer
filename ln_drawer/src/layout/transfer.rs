use ln_world::{HandleAny, HandleGeneric, World};

use crate::{
    tools::pointer::{PointerHit, PointerHover, PointerScroll},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonSelected, SetButtonSelected},
    },
};

/// Very useful when you're trying to mimic a specific widget!
pub struct Transfer<'w>(&'w World, HandleAny, HandleAny);

impl Transfer<'_> {
    pub fn new(world: &World, this: impl HandleGeneric, that: impl HandleGeneric) -> Transfer<'_> {
        Transfer(world, this.untyped(), that.untyped())
    }

    pub fn build<I: Send + 'static, O: Send + 'static>(
        &self,
        f: impl Fn(&I) -> O + Send + 'static,
    ) -> &Self {
        let &Transfer(world, this, that) = self;
        world.observer(this, move |i: &I, world| {
            world.queue_trigger(that, f(i));
        });
        self
    }

    pub fn set_widget_rectangle(&self) -> &Self {
        self.build(|&SetWidgetRectangle(val)| SetWidgetRectangle(val))
    }

    pub fn set_widget_visible(&self) -> &Self {
        self.build(|&SetWidgetVisible(val)| SetWidgetVisible(val))
    }

    pub fn button_selected(&self) -> &Self {
        self.build(|&ButtonSelected(val)| ButtonSelected(val))
    }

    pub fn set_button_selected(&self) -> &Self {
        self.build(|&SetButtonSelected(val)| SetButtonSelected(val))
    }

    pub fn pointer_hit(&self) -> &Self {
        self.build(|&v: &PointerHit| v)
    }

    pub fn pointer_hover(&self) -> &Self {
        self.build(|&v: &PointerHover| v)
    }

    pub fn pointer_scroll(&self) -> &Self {
        self.build(|&v: &PointerScroll| v)
    }
}
