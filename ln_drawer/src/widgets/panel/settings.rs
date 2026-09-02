use cosmic_text::{Attrs, Metrics, Weight};
use ln_world::{Handle, HandleAny, HandleGeneric, World};

use crate::{
    i18n::tr,
    layer::wrapper::{BrushConfigurationChanged, BrushMode, LayerWrapper},
    layout::{
        luni::{
            LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex, LuniParent,
            LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::{Axis, Rectangle},
    theme::Theme,
    widgets::{
        container::Container,
        echo::EchoWidget,
        renderer::text::{SetText, Text},
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
    },
};

pub fn panel_settings(world: &World, panel: Handle<Container>) {
    let theme = world.single_fetch::<Theme>().unwrap();

    let label1_frame = world.insert(EchoWidget);
    let label1 = world.insert(Text {
        text: tr("settings.brush.title"),
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

    // Flow //
    let flow_frame = world.insert(EchoWidget);
    let flow_label = option_label(world, String::new(), flow_frame.untyped());
    let flow_desc = option_desc(world, String::new(), flow_frame.untyped());
    let (flow_slider, flow_slider_label) = option_slider(world, flow_frame.untyped());
    world.observer(flow_slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.flow.scale = value;
        layer.tint_brush.flow.w = value;
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    // Blur Kernel Sigma //
    let sigma_frame = world.insert(EchoWidget);
    let sigma_label = option_label(world, String::new(), sigma_frame.untyped());
    let sigma_desc = option_desc(world, String::new(), sigma_frame.untyped());
    let (sigma_slider, sigma_slider_label) = option_slider(world, sigma_frame.untyped());
    world.observer(sigma_slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.blur_brush.sigma.scale = value * 3.0;
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    // Softness //
    let softness_frame = world.insert(EchoWidget);
    let softness_label = option_label(world, String::new(), softness_frame.untyped());
    let softness_desc = option_desc(world, String::new(), softness_frame.untyped());
    let (softness_slider, softness_slider_label) = option_slider(world, softness_frame.untyped());
    world.observer(softness_slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.softness.scale = 1. - value;
        layer.blur_brush.softness.scale = 1. - value;
        layer.tint_brush.softness.scale = 1. - value;
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();

        let mut flow_label = world.fetch_mut(flow_label).unwrap();
        let mut flow_desc = world.fetch_mut(flow_desc).unwrap();
        let mut sigma_label = world.fetch_mut(sigma_label).unwrap();
        let mut sigma_desc = world.fetch_mut(sigma_desc).unwrap();
        let mut softness_label = world.fetch_mut(softness_label).unwrap();
        let mut softness_desc = world.fetch_mut(softness_desc).unwrap();

        // Flow //
        flow_label.set_text(tr("settings.brush.flow.label"));
        flow_desc.set_text(tr("settings.brush.flow.desc"));
        let flow = match layer.brush_mode {
            BrushMode::Round | BrushMode::Blur => layer.round_brush.flow.scale,
            BrushMode::Tint => layer.tint_brush.flow.w,
        };
        world.queue_trigger(flow_slider, SetSliderValue(flow));
        world.queue_trigger(flow_slider_label, SetText(format!("{flow:.2}")));

        // Sigma //
        sigma_label.set_text(tr("settings.brush.sigma.label"));
        sigma_desc.set_text(tr("settings.brush.sigma.desc"));
        let sigma = layer.blur_brush.sigma.scale;
        world.queue_trigger(sigma_slider, SetSliderValue(sigma / 3.0));
        world.queue_trigger(sigma_slider_label, SetText(format!("{sigma:.2}")));

        // Softness
        softness_label.set_text(tr("settings.brush.softness.label"));
        softness_desc.set_text(tr("settings.brush.softness.desc"));
        let softness = match layer.brush_mode {
            BrushMode::Round => 1. - layer.round_brush.softness.scale,
            BrushMode::Blur => 1. - layer.blur_brush.softness.scale,
            BrushMode::Tint => 1. - layer.tint_brush.softness.scale,
        };
        world.queue_trigger(softness_slider, SetSliderValue(softness));
        world.queue_trigger(softness_slider_label, SetText(format!("{softness:.2}")));
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
                sigma_frame.untyped(),
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

fn option_label(world: &World, text: String, option1_frame: HandleAny) -> Handle<Text> {
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

fn option_desc(world: &World, text: String, option1_frame: HandleAny) -> Handle<Text> {
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

fn option_slider(world: &World, option1_frame: HandleAny) -> (Handle<Slider>, Handle<SliderLabel>) {
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
