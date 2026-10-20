use cosmic_text::{Align, Attrs, Metrics};
use glam::{DVec2, I64Vec2, IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    i18n::trp,
    layer::{
        brush::param::BrushParamKey,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::{Axis, Rectangle},
    theme::Theme,
    widgets::{
        common::WidgetCommon,
        container::{ContainerEcho, ContainerWithCamera},
        panel::{Panel, brush_panel::BrushPanel, color_picker::LayerOklabPalette},
        renderer::text::{SetText, Text},
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
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

        let tool = world.insert(ToolBar {
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
                    tool.untyped(),
                    LuniChild {
                        basis: Some(40),
                        ..Default::default()
                    },
                ),
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
                        basis: Some(20),
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

        world.named("layer_init");
        world.queue_trigger(page, BrushConfigurationChanged);
    }
}

/// the top one
pub struct ToolBar {
    pub common: WidgetCommon,
}

impl Element for ToolBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let page = world.single::<LayerPage>();

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

        let slider = world.insert(Slider {
            common: WidgetCommon {
                camera,
                visible: true,
            },
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
            axis: Axis::Right,
            value: 0.67,
            pressed: false,
        });

        let slider_label = world.insert(SliderLabel {
            text: String::new(),
            clockwise: true,
            source: slider,
            hover: false,
            visible: true,
            camera,
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: container.untyped(),
            target: panel.untyped(),
        });

        world.insert(LuniFlex {
            parent: (
                container.untyped(),
                LuniParent {
                    axis: LuniAxis::Row,
                    distribution: LuniDistribution::Center,
                    padding: LuniRect::default(),
                    gap: 0,
                    template: LuniChildTemplate::default(),
                },
            ),
            children: vec![(
                slider_label.untyped(),
                LuniChild {
                    shrink: Some(1.0),
                    ..Default::default()
                },
            )],
        });

        world.observer(slider, move |&SliderValue(value), world| {
            let mut page = world.single_fetch_mut::<LayerPage>();
            let scale = (value.exp2() - 1.) * 127.5 + 0.5;
            if let Some(param) = page.active_mut().scalar_mut(BrushParamKey::Size) {
                param.scale = scale;
            }

            world.queue_trigger(page.handle(), BrushConfigurationChanged);
        });

        world.observer(page, move |&BrushConfigurationChanged, world| {
            let page = world.single_fetch::<LayerPage>();
            let value = page
                .active()
                .scalar(BrushParamKey::Size)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            world.trigger(slider, &SetSliderValue(((value - 0.5) / 127.5 + 1.).log2()));
            world.queue_trigger(slider_label, SetText(format!("{value:.2} px")));
        });

        world.dependency(container, this);
    }
}

/// the bottom one
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
                font_size: 12.0,
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
