use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::widgets::{
    SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
    button::{ButtonSelected, SetButtonSelected},
    slider::{SetSliderValue, SliderValue},
    tabs::{SetTabsActive, TabsActive},
};

/// Utility to build certain mysterious echo over specific element,
/// who receive all kinds of command event and emit out
/// correlated notification event immediately.
pub struct Echo<'w>(&'w World, HandleAny);

#[expect(unused)]
pub struct EchoAll;
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

    #[expect(unused)]
    pub fn slider_value(&self) -> &Self {
        self.build(|&SetSliderValue(val)| SliderValue(val))
    }

    #[expect(unused)]
    pub fn tabs_active(&self) -> &Self {
        self.build(|&SetTabsActive(val)| TabsActive(val))
    }

    #[expect(unused)]
    pub fn button_selected(&self) -> &Self {
        self.build(|&SetButtonSelected(val)| ButtonSelected(val))
    }

    #[expect(unused)]
    pub fn all(&self) {
        self.widget_rectangle();
        self.widget_visible();
        self.slider_value();
        self.tabs_active();
        self.button_selected();
    }
}

impl Element for EchoAll {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        Echo::new(world, this).all();
    }
}

impl Element for EchoWidget {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        Echo::new(world, this).widget_rectangle().widget_visible();
    }
}
