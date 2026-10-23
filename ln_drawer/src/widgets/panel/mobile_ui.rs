use ln_world::{Element, Handle, World};

use crate::{
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    lnwin::LnwindowRect,
    widgets::{common::WidgetCommon, panel::side_docker::SideDocker},
};

pub struct MobileUi {
    pub common: WidgetCommon,
}

impl Element for MobileUi {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("mobile_ui");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let rect = world.single_fetch::<LnwindowRect>().0;

        let page = world.insert(LayerPage::new(world, self.common, rect));
        world.insert(SideDocker {
            common: self.common,
            rect,
        });

        world.scope("layer_init");
        world.queue_trigger(page, BrushConfigurationChanged);
    }
}
