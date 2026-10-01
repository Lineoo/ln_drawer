use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::transform::{Transform, TransformEdge, TransformValue},
    lnwin::Lnwindow,
    measures::Rectangle,
    widgets::{
        common::WidgetCommon,
        panel::{Panel, color_picker::LayerOklabPalette},
    },
};

pub struct DesktopUi {
    pub common: WidgetCommon,
}

impl Element for DesktopUi {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();

        let back = world.insert(Panel {
            rect: Rectangle::default(),
            visible: self.common.visible,
            shadow: true,
            camera: self.common.camera,
        });

        let oklab = world.insert(LayerOklabPalette {
            common: WidgetCommon {
                visible: self.common.visible,
                camera: self.common.camera,
            },
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 1.0,
                    offset: -410,
                },
                down: TransformEdge {
                    anchor: 0.0,
                    offset: 10,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: -10,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -10,
                },
            },
            source: lnwindow.untyped(),
            target: back.untyped(),
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 0,
                },
                down: TransformEdge {
                    anchor: 1.0,
                    offset: -300,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: 0,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: 0,
                },
            },
            source: back.untyped(),
            target: oklab.untyped(),
        });

        world.dependency(back, this);
        world.dependency(oklab, this);
    }
}
