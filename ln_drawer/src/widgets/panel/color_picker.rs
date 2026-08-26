use std::sync::Arc;

use cosmic_text::{Attrs, Metrics, Weight};
use glam::{IVec2, UVec2, Vec2};
use ln_world::{Descriptor, Handle, World};
use palette::{Hsla, IntoColor, Oklab, RgbHue, Srgba};

use crate::{
    layer::wrapper::{BrushConfigurationChanged, BrushMode, LayerWrapper},
    layout::{
        luni::{
            LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex, LuniParent,
            LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::{Axis, Rectangle},
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        echo::EchoWidget,
        palette::{
            hsl::{ColorHsla, HslPanel, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        panel::Panel,
        renderer::{
            svg::svg_render,
            text::{SetText, Text},
        },
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
        tabs::Tabs,
    },
};

pub struct ColorPicker(pub Handle<ToggleButton>);
impl Descriptor for ColorPicker {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let toggle_button = self.0;

        let toggle_button_color_icon = world.build(RoundedRectDescriptor {
            order: 21,
            color: Srgba::new(0.9, 0.7, 0.7, 1.0),
            value: 10.0,
            shrink: 10.0,
            shadow_offset: Vec2::ZERO,
            vertex_extend: 20,
            ..Default::default()
        });

        world.observer(toggle_button, move |&SetWidgetRectangle(rect), world| {
            let transform = Transform {
                value: TransformValue::anchor(
                    (0.5, 0.5),
                    Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
                ),
                source: toggle_button.untyped(),
                target: toggle_button_color_icon.untyped(),
            };

            let target = transform.value.compute(rect);

            world.queue_trigger(toggle_button_color_icon, SetWidgetRectangle(target));
        });

        let tab_palette_hsl = world.insert(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        palette_hsl(world, tab_palette_hsl);

        let tab_palette_oklch = world.insert(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        palette_oklab(world, tab_palette_oklch);

        let tab_settings = world.insert(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        settings(world, tab_settings);

        let tabs = world.insert(Tabs {
            active: 0,
            rect: Rectangle::default(),
            visible: false,
            tabs: vec![
                (
                    ButtonImage {
                        transform: TransformValue::anchor(
                            (0.5, 0.5),
                            Rectangle::new_half(IVec2::ZERO, UVec2::splat(12)),
                        ),
                        bytes: Arc::new(image::DynamicImage::from(svg_render(
                            include_bytes!("../../../res/interface/palette.svg"),
                            1.0,
                        ))),
                    },
                    tab_palette_hsl.untyped(),
                ),
                (
                    ButtonImage {
                        transform: TransformValue::anchor(
                            (0.5, 0.5),
                            Rectangle::new_half(IVec2::ZERO, UVec2::splat(12)),
                        ),
                        bytes: Arc::new(image::DynamicImage::from(svg_render(
                            include_bytes!("../../../res/interface/palette.svg"),
                            1.0,
                        ))),
                    },
                    tab_palette_oklch.untyped(),
                ),
                (
                    ButtonImage {
                        transform: TransformValue::anchor(
                            (0.5, 0.5),
                            Rectangle::new_half(IVec2::ZERO, UVec2::splat(12)),
                        ),
                        bytes: Arc::new(image::DynamicImage::from(svg_render(
                            include_bytes!("../../../res/interface/settings.svg"),
                            1.0,
                        ))),
                    },
                    tab_settings.untyped(),
                ),
            ],
        });

        let layer = world.single::<LayerWrapper>().unwrap();
        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(layer).unwrap();
            let mut toggle_button_color_icon = world.fetch_mut(toggle_button_color_icon).unwrap();
            let color = layer.round_brush.color;
            toggle_button_color_icon.desc.color = color.into_color();
            // trigger palette_hsl SetPaletteHsl
        });

        world.observer(toggle_button, move |&SetWidgetRectangle(rect), world| {
            let transform = TransformValue::anchor(
                (1.0, 0.5),
                Rectangle::new_half(IVec2::new(192 + 20, 0), UVec2::new(192, 144)),
            );
            let rect = transform.compute(rect);
            world.queue_trigger(tabs, SetWidgetRectangle(rect));
        });

        world.observer(toggle_button, move |&ButtonSelected(selected), world| {
            world.queue_trigger(toggle_button, SetButtonSelected(selected));
            world.queue_trigger(tabs, SetWidgetVisible(selected));
        });
    }
}

fn palette_hsl(world: &World, bg: Handle<Panel>) {
    let panel = world.insert(HslPanel {
        rect: Rectangle::default(),
        color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
        enabled: true,
    });

    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
        ),
        source: bg.untyped(),
        target: panel.untyped(),
    });

    let layer = world.single::<LayerWrapper>().unwrap();
    world.observer(panel, move |&ColorHsla(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.color = color.into_color();
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let hsla = layer.round_brush.color.into_color();
        world.trigger(panel, &SetColorHsla(hsla));
    });
}

fn palette_oklab(world: &World, bg: Handle<Panel>) {
    let polar = world.insert(OklabPolar {
        rect: Rectangle::default(),
        color: Oklab::default(),
        enabled: true,
    });
    let bar = world.insert(OklabBar {
        rect: Rectangle::default(),
        color: Oklab::default(),
        enabled: true,
    });

    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::new(-30, 0), UVec2::splat(100)),
        ),
        source: bg.untyped(),
        target: polar.untyped(),
    });
    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::new(110, 0), UVec2::new(20, 100)),
        ),
        source: bg.untyped(),
        target: bar.untyped(),
    });

    let layer = world.single::<LayerWrapper>().unwrap();
    world.observer(polar, move |&ColorOklab(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.color = color.into_color();
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(bar, move |&ColorOklab(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.color = color.into_color();
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let oklab = layer.round_brush.color.into_color();
        world.trigger(polar, &SetColorOklab(oklab));
        world.trigger(bar, &SetColorOklab(oklab));
    });
}

fn settings(world: &World, panel: Handle<Panel>) {
    let theme = world.single_fetch::<Theme>().unwrap();

    let label1_frame = world.insert(EchoWidget);
    let label1 = world.insert(Text {
        text: String::from("选项标签"),
        metrics: Metrics {
            font_size: 14.0,
            line_height: 18.0,
        },
        attrs: Attrs::new().weight(Weight::BOLD),
        color: theme.significant_color,
        ..Default::default()
    });
    world.insert(Transform {
        value: TransformValue::anchor((0.0, 0.0), Rectangle::new_extend(16, 0, 56, 18)),
        source: label1_frame.untyped(),
        target: label1.untyped(),
    });

    let layer = world.single::<LayerWrapper>().unwrap();

    let flow_frame = world.insert(EchoWidget);
    let flow_label = option_label(world, String::new(), flow_frame.untyped());
    let flow_desc = option_desc(world, String::new(), flow_frame.untyped());
    let (flow_slider, flow_slider_label) = option_slider(world, flow_frame.untyped());
    world.observer(flow_slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        match layer.brush_mode {
            BrushMode::Round => layer.round_brush.flow.scale = value,
            BrushMode::Blur => layer.blur_brush.sigma.scale = value * 3.0,
        };
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    let softness_frame = world.insert(EchoWidget);
    let softness_label = option_label(world, String::new(), softness_frame.untyped());
    let softness_desc = option_desc(world, String::new(), softness_frame.untyped());
    let (softness_slider, softness_slider_label) = option_slider(world, softness_frame.untyped());
    world.observer(softness_slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        match layer.brush_mode {
            BrushMode::Round => layer.round_brush.softness.scale = 1. - value,
            BrushMode::Blur => layer.blur_brush.softness.scale = 1. - value,
        };
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();

        let mut flow_label = world.fetch_mut(flow_label).unwrap();
        let mut flow_desc = world.fetch_mut(flow_desc).unwrap();

        let value = match layer.brush_mode {
            BrushMode::Round => layer.round_brush.flow.scale,
            BrushMode::Blur => layer.blur_brush.sigma.scale / 3.0,
        };

        world.queue_trigger(flow_slider, SetSliderValue(value));
        world.queue_trigger(flow_slider_label, SetText(format!("{value:.2}")));

        let (label, desc) = match layer.brush_mode {
            BrushMode::Round => ("流量", "笔刷每步流量（范围：[0, 1]）"),
            BrushMode::Blur => ("模糊标准差", "卷积核应用半径：r = σ * 3（范围：[0, 1]）"),
        };

        if flow_label.text != label {
            flow_label.text = label.into();
            flow_label.outdated = true;
        }

        if flow_desc.text != desc {
            flow_desc.text = desc.into();
            flow_desc.outdated = true;
        }

        let mut softness_label = world.fetch_mut(softness_label).unwrap();
        let mut softness_desc = world.fetch_mut(softness_desc).unwrap();

        let value = match layer.brush_mode {
            BrushMode::Round => 1. - layer.round_brush.softness.scale,
            BrushMode::Blur => 1. - layer.blur_brush.softness.scale,
        };

        world.queue_trigger(softness_slider, SetSliderValue(value));
        world.queue_trigger(softness_slider_label, SetText(format!("{value:.2}")));

        let (label, desc) = ("硬度", "三次多项式平滑（范围：[0, 1]）");

        if softness_label.text != label {
            softness_label.text = label.into();
            softness_label.outdated = true;
        }

        if softness_desc.text != desc {
            softness_desc.text = desc.into();
            softness_desc.outdated = true;
        }
    });

    world.insert(LuniFlex {
        parent: (
            panel.untyped(),
            LuniParent {
                axis: LuniAxis::Column,
                distribution: LuniDistribution::FlexStart,
                padding: LuniRect {
                    left: 12,
                    bottom: 4,
                    right: 12,
                    top: 4,
                },
                gap: 4,
                template: LuniChildTemplate::default(),
            },
        ),
        children: vec![
            (
                label1_frame.untyped(),
                LuniChild {
                    basis: Some(48),
                    ..Default::default()
                },
            ),
            (
                flow_frame.untyped(),
                LuniChild {
                    basis: Some(108),
                    ..Default::default()
                },
            ),
            (
                softness_frame.untyped(),
                LuniChild {
                    basis: Some(108),
                    ..Default::default()
                },
            ),
        ],
    });
}

fn option_label(world: &World, text: String, option1_frame: Handle) -> Handle<Text> {
    let theme = world.single_fetch::<Theme>().unwrap();

    let label = world.insert(Text {
        text,
        metrics: Metrics {
            font_size: 16.0,
            line_height: 20.0,
        },
        color: theme.symbolic_color,
        ..Default::default()
    });

    world.insert(Transform {
        value: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 56,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -36,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: -72,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: -16,
            },
        },
        source: option1_frame,
        target: label.untyped(),
    });

    label
}

fn option_desc(world: &World, text: String, option1_frame: Handle) -> Handle<Text> {
    let theme = world.single_fetch::<Theme>().unwrap();

    let label = world.insert(Text {
        text,
        metrics: Metrics {
            font_size: 12.0,
            line_height: 14.0,
        },
        color: theme.significant_color,
        ..Default::default()
    });

    world.insert(Transform {
        value: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 56,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -72,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: -72,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: -36,
            },
        },
        source: option1_frame,
        target: label.untyped(),
    });

    label
}

fn option_slider(world: &World, option1_frame: Handle) -> (Handle<Slider>, Handle<SliderLabel>) {
    let slider = world.insert(Slider {
        value: 0.5,
        axis: Axis::Right,
        rect: Rectangle::default(),
        pressed: false,
    });

    world.insert(Transform {
        value: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 56,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -108,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: -72,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: -72,
            },
        },
        source: option1_frame,
        target: slider.untyped(),
    });

    let slider_label = world.insert(SliderLabel {
        text: String::new(),
        clockwise: false,
        source: slider,
        hover: false,
        visible: true,
    });

    (slider, slider_label)
}
