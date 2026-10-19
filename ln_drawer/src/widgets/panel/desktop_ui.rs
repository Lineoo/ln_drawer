use cosmic_text::{Align, Attrs, Metrics};
use glam::{DVec2, I64Vec2, IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    i18n::trp,
    layer::wrapper::LayerPage,
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::Rectangle,
    theme::Theme,
    widgets::{
        common::WidgetCommon,
        container::{ContainerEcho, ContainerWithCamera},
        panel::{Panel, brush_panel::BrushPanel, color_picker::LayerOklabPalette},
        renderer::text::Text,
    },
};

pub struct DesktopUi {
    pub common: WidgetCommon,
}

impl Element for DesktopUi {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();

        let workspace = world.insert(MainWorkspace {
            common: self.common,
        });

        let status = world.insert(StatusBar {
            common: self.common,
        });

        world.insert(LuniFlex {
            parent: (
                lnwindow.untyped(),
                LuniParent {
                    axis: LuniAxis::Column,
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
                    workspace.untyped(),
                    LuniChild {
                        grow: Some(1.0),
                        shrink: Some(1.0),
                        ..Default::default()
                    },
                ),
                (
                    status.untyped(),
                    LuniChild {
                        basis: Some(28),
                        ..Default::default()
                    },
                ),
            ],
        });

        world.dependency(workspace, this);
    }
}

pub struct MainWorkspace {
    pub common: WidgetCommon,
}

impl Element for MainWorkspace {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            anchor: DVec2::new(0.5, 0.5),
            scroll: I64Vec2::ZERO,
        });
        world.insert(ContainerEcho { container });
        Transfer::new(world, this, container)
            .set_widget_rectangle()
            .set_widget_visible();

        let page = world.insert(LayerPage::new(
            world,
            WidgetCommon {
                camera,
                ..self.common
            },
        ));

        let command_center = world.insert(CommandCenter {
            common: WidgetCommon {
                camera,
                ..self.common
            },
        });

        world.insert(LuniFlex {
            parent: (
                container.untyped(),
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
                    command_center.untyped(),
                    LuniChild {
                        basis: Some(400),
                        shrink: Some(1.0),
                        ..Default::default()
                    },
                ),
            ],
        });

        world.dependency(container, this);
        world.dependency(page, this);
        world.dependency(command_center, this);
    }
}

pub struct StatusBar {
    pub common: WidgetCommon,
}

impl Element for StatusBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();

        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            anchor: DVec2::new(0.5, 0.5),
            scroll: I64Vec2::ZERO,
        });
        world.insert(ContainerEcho { container });
        Transfer::new(world, this, container)
            .set_widget_rectangle()
            .set_widget_visible();

        let panel = world.insert(Panel {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
            },
            rect: Rectangle::default(),
            shadow: false,
        });

        let text = world.insert(Text {
            text: trp("title", &[("version", env!("CARGO_PKG_VERSION"))]).into(),
            metrics: Metrics {
                font_size: 16.0,
                line_height: 18.0,
            },
            align: Align::Center,
            attrs: Attrs::new(),
            color: theme.highlight_color,
            camera: Some(camera),
            visible: self.common.visible,
            order: 120,
            ..Default::default()
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: container.untyped(),
            target: panel.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::new(100, 10)),
            ),
            source: container.untyped(),
            target: text.untyped(),
        });

        world.dependency(container, this);
        world.dependency(text, this);
    }
}

pub struct CommandCenter {
    pub common: WidgetCommon,
}

impl Element for CommandCenter {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            anchor: DVec2::new(0.5, 0.5),
            scroll: I64Vec2::ZERO,
        });
        world.insert(ContainerEcho { container });
        Transfer::new(world, this, container)
            .set_widget_rectangle()
            .set_widget_visible();

        let panel = world.insert(Panel {
            common: WidgetCommon {
                camera,
                ..self.common
            },
            rect: Rectangle::default(),
            shadow: true,
        });

        let oklab = world.insert(LayerOklabPalette {
            common: WidgetCommon {
                camera,
                ..self.common
            },
            rect: Rectangle::default(),
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        let brush = world.insert(BrushPanel {
            common: WidgetCommon {
                camera,
                ..self.common
            },
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: container.untyped(),
            target: panel.untyped(),
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
            source: container.untyped(),
            target: oklab.untyped(),
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 0,
                },
                down: TransformEdge {
                    anchor: 0.0,
                    offset: 0,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: 0,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -300,
                },
            },
            source: container.untyped(),
            target: brush.untyped(),
        });

        world.dependency(panel, this);
        world.dependency(oklab, this);
    }
}
