use glam::Vec4;
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layer::wrapper::LayerPage,
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
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
        let lnwindow = world.single::<Lnwindow>();

        let page = world.insert(LayerPage::new(world));

        let right_side = world.insert(Panel {
            common: self.common,
            rect: Rectangle::default(),
            shadow: true,
        });

        let oklab = world.insert(LayerOklabPalette {
            common: self.common,
            rect: Rectangle::default(),
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        world.insert(LuniFlex {
            parent: (
                lnwindow.untyped(),
                LuniParent {
                    axis: LuniAxis::Row,
                    distribution: LuniDistribution::FlexStart,
                    padding: LuniRect::default(),
                    gap: 0,
                    template: LuniChildTemplate {
                        align: LuniAlign::Stretch,
                        ..Default::default()
                    },
                },
            ),
            children: vec![
                (
                    page.untyped(),
                    LuniChild {
                        basis: Some(400),
                        grow: Some(1.0),
                        shrink: Some(1.0),
                        ..Default::default()
                    },
                ),
                (
                    right_side.untyped(),
                    LuniChild {
                        basis: Some(400),
                        shrink: Some(1.0),
                        ..Default::default()
                    },
                ),
            ],
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
            source: right_side.untyped(),
            target: oklab.untyped(),
        });

        world.dependency(page, this);
        world.dependency(right_side, this);
        world.dependency(oklab, this);
    }
}
