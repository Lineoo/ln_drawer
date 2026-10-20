use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::transform::{Transform, TransformValue},
    lnwin::Lnwindow,
    widgets::{common::WidgetCommon, panel::side_docker::side_docker},
};

pub struct MobileUi {
    pub common: WidgetCommon,
}

impl Element for MobileUi {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();

        let page = world.insert(LayerPage::new(world, self.common));

        world.insert(Transform {
            value: TransformValue::copy(),
            source: lnwindow.untyped(),
            target: page.untyped(),
        });

        let camera = self.common.camera;
        world.queue(move |world| side_docker(world, camera));

        world.dependency(page, this);

        world.named("layer_init");
        world.queue_trigger(page, BrushConfigurationChanged);
    }
}
