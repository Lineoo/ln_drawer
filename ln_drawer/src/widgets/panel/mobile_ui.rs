use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layer::wrapper::LayerPage,
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

        let page = world.insert(LayerPage::new(world));

        world.insert(Transform {
            value: TransformValue::shrink(50, 50),
            source: lnwindow.untyped(),
            target: page.untyped(),
        });

        side_docker(world, self.common.camera);

        world.dependency(page, this);
    }
}
