use cosmic_text::{Align, Attrs, Metrics};
use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    i18n::trp,
    layer::{
        brush::param::BrushParamKey,
        input::LayerInput,
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
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonClick, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{ContainerAttr, ContainerEcho, ContainerWithCamera},
        panel::{
            Panel, PanelAttr, brush_panel::BrushListPanel, color_picker::LayerOklabPalette,
            debug_panel::DebugPanel, layer_selection::LayerSelectionPanel, settings::SettingPanel,
        },
        renderer::{
            svg::svg_render,
            text::{SetText, Text},
        },
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
        tabs::{SetTabsActive, Tabs},
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

        world.named("layer_init");
        world.queue(|world| {
            world.trigger(world.single::<LayerPage>(), &BrushConfigurationChanged);
        });
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
            attr: ContainerAttr::default(),
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

/// the top one
pub struct ToolBar {
    pub common: WidgetCommon,
}

impl Element for ToolBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let page = world.single::<LayerPage>();
        let theme = world.single_fetch::<Theme>();

        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            attr: ContainerAttr {
                // cause we need the slider label to display
                clip: false,
                ..Default::default()
            },
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
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let docker_button = |image_bytes| {
            world.insert(ToggleButton {
                common: WidgetCommon {
                    camera,
                    visible: self.common.visible,
                },
                rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
                theme: theme.button(),
                image: Some(svg_render(image_bytes, 8)),
                selected: false,
                hovering: false,
            })
        };

        let eraser = docker_button(include_bytes!("../../../res/interface/eraser.svg"));
        let undo = docker_button(include_bytes!("../../../res/interface/undo-2.svg"));
        let redo = docker_button(include_bytes!("../../../res/interface/redo-2.svg"));
        let touch = docker_button(include_bytes!("../../../res/interface/pointer.svg"));

        let slider = world.insert(Slider {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
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
            visible: self.common.visible,
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
                    gap: 4,
                    template: LuniChildTemplate {
                        basis: 36,
                        cross: 36,
                        shrink: 1.0,
                        align: LuniAlign::Center,
                        ..Default::default()
                    },
                },
            ),
            children: vec![
                (undo.untyped(), LuniChild::default()),
                (redo.untyped(), LuniChild::default()),
                (eraser.untyped(), LuniChild::default()),
                (
                    slider.untyped(),
                    LuniChild {
                        basis: Some(200),
                        shrink: Some(1.0),
                        ..Default::default()
                    },
                ),
                (touch.untyped(), LuniChild::default()),
            ],
        });

        world.observer(eraser, move |&ButtonSelected(val), world| {
            let mut page = world.fetch_mut(page);
            page.active_mut().set_toggle(BrushParamKey::Erase, val);
            world.queue_trigger(page.handle(), BrushConfigurationChanged);
        });

        world.observer(touch, move |&ButtonSelected(val), world| {
            let mut input = world.single_fetch_mut::<LayerInput>();
            input.touch_draw = val;
            world.queue_trigger(touch, SetButtonSelected(val));
        });

        world.observer(page, move |&BrushConfigurationChanged, world| {
            let is_eraser = {
                let layer = world.fetch(page);
                layer.active().toggle(BrushParamKey::Erase).unwrap_or(false)
            };
            world.trigger(eraser, &SetButtonSelected(is_eraser));
        });

        world.observer(undo, move |&ButtonClick, world| {
            let mut page = world.fetch_mut(page);
            page.undo();
        });

        world.observer(redo, move |&ButtonClick, world| {
            let mut page = world.fetch_mut(page);
            page.redo();
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
            attr: ContainerAttr::default(),
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
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let text = world.insert(Text {
            text: trp("title", &[("version", env!("CARGO_PKG_VERSION"))]).into(),
            metrics: Metrics {
                font_size: 12.0,
                line_height: 18.0,
            },
            align: Align::Center,
            attrs: Attrs::new(),
            color: theme.gray3,
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
            attr: ContainerAttr::default(),
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
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let oklab = world.insert(LayerOklabPalette {
            common: WidgetCommon {
                camera,
                ..self.common
            },
            rect: Rectangle::default(),
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        let verbose = world.insert(VerbosePanel {
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
            target: verbose.untyped(),
        });

        world.dependency(panel, this);
        world.dependency(oklab, this);
    }
}

pub struct VerbosePanel {
    pub common: WidgetCommon,
}

impl Element for VerbosePanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let tab_brush_list = world.insert(BrushListPanel {
            common: self.common,
        });

        let tab_settings = world.insert(SettingPanel {
            common: self.common,
        });

        let tab_layer_selection = world.insert(LayerSelectionPanel {
            common: self.common,
        });

        let tab_debug = world.insert(DebugPanel {
            common: self.common,
        });

        let tabs = world.insert(Tabs {
            active: 0,
            rect: Rectangle::default(),
            visible: self.common.visible,
            tabs: vec![
                (
                    svg_render(include_bytes!("../../../res/interface/brush.svg"), 12),
                    tab_brush_list.untyped(),
                ),
                (
                    svg_render(include_bytes!("../../../res/interface/settings.svg"), 12),
                    tab_settings.untyped(),
                ),
                (
                    svg_render(include_bytes!("../../../res/interface/layers.svg"), 12),
                    tab_layer_selection.untyped(),
                ),
                (
                    svg_render(include_bytes!("../../../res/interface/bug.svg"), 12),
                    tab_debug.untyped(),
                ),
            ],
            camera: self.common.camera,
        });

        Transfer::new(world, this, tabs)
            .set_widget_rectangle()
            .set_widget_visible();

        world.dependency(tab_brush_list, this);
        world.dependency(tab_settings, this);
        world.dependency(tab_layer_selection, this);
        world.dependency(tab_debug, this);
        world.dependency(tabs, this);

        // initialize layout
        world.queue(move |world| {
            let this = world.fetch(tabs);
            world.queue_trigger(tabs, SetWidgetRectangle(this.rect));
            world.queue_trigger(tabs, SetWidgetVisible(this.visible));
            world.queue_trigger(tabs, SetTabsActive(this.active));
        });
    }
}
