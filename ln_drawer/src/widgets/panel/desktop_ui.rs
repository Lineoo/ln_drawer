use cosmic_text::{Align, Attrs, Metrics};
use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, World};

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
        transform::{Transform, TransformEdge, TransformValue},
        val::{Val, ValChanged},
    },
    lnwin::LnwindowRect,
    measures::{Axis, Rectangle},
    theme::ThemeVal,
    widgets::{
        button::{ButtonClick, ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{ContainerAttr, ContainerEcho, ContainerWithCamera},
        panel::{
            Panel, PanelAttr, brush_panel::BrushListPanel, color_picker::LayerOklabPalette,
            debug_panel::DebugPanel, layer_selection::LayerSelectionPanel, settings::SettingPanel,
        },
        renderer::{
            svg::svg_render,
            text::{SetText, Text, TextAttr},
        },
        slider::{Slider, SliderLabel, SliderValue},
        tabs::{TabEntry, Tabs},
    },
};

pub struct DesktopUi {
    pub common: WidgetCommon,
}

impl Element for DesktopUi {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("desktop_ui");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let root = world.single_fetch::<LnwindowRect>().0;

        let mut luni = LuniFlex::new(
            root,
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
        );

        let tool_rect = world.build(luni.child(LuniChild {
            basis: Some(40),
            ..Default::default()
        }));
        let workspace_rect = world.build(luni.child(LuniChild {
            grow: Some(1.0),
            shrink: Some(1.0),
            ..Default::default()
        }));
        let status_rect = world.build(luni.child(LuniChild {
            basis: Some(20),
            ..Default::default()
        }));

        // reordered since page system is relied by singleton

        world.insert(MainWorkspace {
            common: self.common,
            rect: workspace_rect,
        });
        world.insert(ToolBar {
            common: self.common,
            rect: tool_rect,
        });
        world.insert(StatusBar {
            common: self.common,
            rect: status_rect,
        });

        world.insert(luni);

        world.scope("layer_init");
        world.queue(|world| {
            world.trigger(world.single::<LayerPage>(), &BrushConfigurationChanged);
        });
    }
}

pub struct MainWorkspace {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for MainWorkspace {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("main_workspace");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let local = world.build(ContainerEcho { container });

        let mut luni = LuniFlex::new(
            local,
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
        );

        let page_rect = world.build(luni.child(LuniChild {
            basis: Some(400),
            grow: Some(1.0),
            shrink: Some(1.0),
            ..Default::default()
        }));
        world.insert(LayerPage::new(world, common, page_rect));

        let command_center_rect = world.build(luni.child(LuniChild {
            basis: Some(400),
            shrink: Some(1.0),
            ..Default::default()
        }));
        world.insert(CommandCenter {
            common,
            rect: command_center_rect,
        });

        world.insert(luni);
    }
}

/// the top one
pub struct ToolBar {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for ToolBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("tool_bar");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let page = world.single::<LayerPage>();
        let theme = world.single_fetch::<ThemeVal>();

        let attr = ContainerAttr {
            // cause we need the slider label to display
            clip: false,
            ..Default::default()
        };
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let local = world.build(ContainerEcho { container });

        world.insert(Panel {
            common,
            rect: local,
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let docker_button = |rect: Handle<Val<Rectangle>>, image_bytes: &[u8]| {
            world.insert(ToggleButton {
                common,
                rect,
                theme: theme.button(),
                image: Some(ButtonImage::new(svg_render(image_bytes), theme.distinct, 8)),
                selected: world.insert(Val::new(false)),
                hovering: world.insert(Val::new(false)),
            })
        };

        let mut luni = LuniFlex::new(
            local,
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
        );

        let undo_rect = world.build(luni.child(LuniChild::default()));
        let undo = docker_button(
            undo_rect,
            include_bytes!("../../../res/interface/undo-2.svg"),
        );
        let redo_rect = world.build(luni.child(LuniChild::default()));
        let redo = docker_button(
            redo_rect,
            include_bytes!("../../../res/interface/redo-2.svg"),
        );
        let eraser_rect = world.build(luni.child(LuniChild::default()));
        let eraser = docker_button(
            eraser_rect,
            include_bytes!("../../../res/interface/eraser.svg"),
        );

        let slider_rect = world.build(luni.child(LuniChild {
            basis: Some(200),
            shrink: Some(1.0),
            ..Default::default()
        }));
        let value = world.insert(Val::new(0.67));
        let slider = world.insert(Slider {
            common: WidgetCommon {
                camera: common.camera,
                visible: self.common.visible,
            },
            rect: slider_rect,
            axis: Axis::Right,
            value,
            pressed: false,
        });

        let slider_label = world.insert(SliderLabel {
            text: String::new(),
            clockwise: true,
            source: slider,
            hover: false,
            common,
        });

        let touch_rect = world.build(luni.child(LuniChild::default()));
        let touch = docker_button(
            touch_rect,
            include_bytes!("../../../res/interface/pointer.svg"),
        );
        let pipette_rect = world.build(luni.child(LuniChild::default()));
        let pipette = docker_button(
            pipette_rect,
            include_bytes!("../../../res/interface/pipette.svg"),
        );

        world.insert(luni);

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

        world.observer(pipette, move |&ButtonSelected(val), world| {
            let mut input = world.single_fetch_mut::<LayerInput>();
            input.hold_pick = val;
            world.queue_trigger(pipette, SetButtonSelected(val));
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
            let slider = world.fetch(slider);
            let mut slider_value = world.fetch_mut(slider.value);
            let value = page
                .active()
                .scalar(BrushParamKey::Size)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            slider_value.val = ((value - 0.5) / 127.5 + 1.).log2();
            world.queue_trigger(slider_value.handle(), ValChanged);
            world.queue_trigger(slider_label, SetText(format!("{value:.2} px")));
        });
    }
}

/// the bottom one
pub struct StatusBar {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for StatusBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("status_bar");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let theme = world.single_fetch::<ThemeVal>();

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let local = world.build(ContainerEcho { container });

        world.insert(Panel {
            common,
            rect: local,
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let text_rect = world.build(Transform::new(
            TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::new(100, 10)),
            ),
            local,
        ));
        world.insert(Text {
            text: trp("title", &[("version", env!("CARGO_PKG_VERSION"))]).into(),
            rect: text_rect,
            attr: TextAttr {
                metrics: Metrics {
                    font_size: 12.0,
                    line_height: 18.0,
                },
                align: Align::Center,
                attrs: Attrs::new(),
                ..Default::default()
            },
            order: 120,
            common,
            modulate: theme.gray3,
            outdated: false,
            canvas_outdated: false,
        });
    }
}

pub struct CommandCenter {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for CommandCenter {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("command_center");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let local = world.build(ContainerEcho { container });

        world.insert(Panel {
            common,
            rect: local,
            attr: PanelAttr {
                shadow: false,
                round: false,
            },
        });

        let oklab_rect = world.build(Transform::new(
            TransformValue {
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
            local,
        ));
        world.insert(LayerOklabPalette {
            common,
            rect: oklab_rect,
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        let verbose_rect = world.build(Transform::new(
            TransformValue {
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
            local,
        ));
        world.insert(VerbosePanel {
            common,
            rect: verbose_rect,
        });
    }
}

pub struct VerbosePanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for VerbosePanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("verbose_panel");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let theme = world.single_fetch::<ThemeVal>();

        let brush_rect = world.insert(Val::new(Rectangle::default()));
        let brush_visible = world.insert(Val::new(false));
        let tab_brush_list = world.insert(BrushListPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: brush_visible,
            },
            rect: brush_rect,
        });

        let settings_rect = world.insert(Val::new(Rectangle::default()));
        let settings_visible = world.insert(Val::new(false));
        let tab_settings = world.insert(SettingPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: settings_visible,
            },
            rect: settings_rect,
        });

        let layer_rect = world.insert(Val::new(Rectangle::default()));
        let layer_visible = world.insert(Val::new(false));
        let tab_layer_selection = world.insert(LayerSelectionPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: layer_visible,
            },
            rect: layer_rect,
        });

        let debug_rect = world.insert(Val::new(Rectangle::default()));
        let debug_visible = world.insert(Val::new(false));
        let tab_debug = world.insert(DebugPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: debug_visible,
            },
            rect: debug_rect,
        });

        world.insert(Tabs {
            common: self.common,
            rect: self.rect,
            tabs: vec![
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/brush.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: brush_rect,
                    visible: brush_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/settings.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: settings_rect,
                    visible: settings_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/layers.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: layer_rect,
                    visible: layer_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/bug.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: debug_rect,
                    visible: debug_visible,
                },
            ],
            active: world.insert(Val::new(0)),
        });

        let _ = (tab_brush_list, tab_settings, tab_layer_selection, tab_debug);
    }
}
