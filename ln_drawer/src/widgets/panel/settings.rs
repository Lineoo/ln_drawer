use cosmic_text::{Attrs, Metrics, Weight};
use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::{
    i18n::tr,
    layer::{
        brush::param::{BrushParamKey, BrushValue, BrushValueMut},
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        echo::EchoWidget,
        luni::{
            LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex, LuniParent,
            LuniRect,
        },
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::{Axis, Rectangle},
    render::camera::Camera,
    theme::Theme,
    widgets::{
        brush_preview::BrushPreview,
        common::WidgetCommon,
        container::{ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerWithCamera},
        renderer::text::{SetText, Text},
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
    },
};

pub struct SettingPanel {
    pub common: WidgetCommon,
}

impl Element for SettingPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let page = world.single_fetch::<LayerPage>();

        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            attr: ContainerAttr::default(),
        });
        let deco = world.insert(ContainerDeco {
            container,
            restriction: Rectangle::default(),
        });
        world.insert(ContainerDecoDropdown {
            height: None,
            container,
            deco,
        });

        Transfer::new(world, this, container)
            .set_widget_rectangle()
            .set_widget_visible();

        // Live brush preview //
        let preview = world.insert(BrushPreview {
            rect: Rectangle::default(),
            outdated: false,
            brush: Some(page.active().dup()),
            visible: false,
            camera,
        });

        let ob = world.observer(page.handle(), move |&BrushConfigurationChanged, world| {
            let mut preview = world.fetch_mut(preview);
            let layer_instance = world.single_fetch::<LayerPage>();
            preview.outdated = true;
            preview.brush = Some(layer_instance.active().dup());
        });

        let settings = world.insert(TextSettingPanel {
            common: WidgetCommon {
                camera,
                ..self.common
            },
        });

        world.dependency(container, this);
        world.dependency(preview, this);
        world.dependency(ob, this);
        world.dependency(settings, this);

        world.insert(LuniFlex {
            parent: (
                container.untyped(),
                LuniParent {
                    axis: LuniAxis::Column,
                    distribution: LuniDistribution::FlexStart,
                    padding: LuniRect::default(),
                    gap: 4,
                    template: LuniChildTemplate::default(),
                },
            ),
            children: vec![
                (
                    preview.untyped(),
                    LuniChild {
                        basis: Some(110),
                        ..Default::default()
                    },
                ),
                (
                    settings.untyped(),
                    LuniChild {
                        basis: Some(200),
                        shrink: Some(1.0),
                        grow: Some(1.0),
                        ..Default::default()
                    },
                ),
            ],
        });
    }
}

pub struct TextSettingPanel {
    pub common: WidgetCommon,
}

impl Element for TextSettingPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let layer = world.single::<LayerPage>();

        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            attr: ContainerAttr::default(),
        });
        let deco = world.insert(ContainerDeco {
            container,
            restriction: Rectangle::default(),
        });
        world.insert(ContainerDecoDropdown {
            height: Some((108 + 4) * 7 + 48 + 8),
            container,
            deco,
        });

        Transfer::new(world, this, container)
            .set_widget_rectangle()
            .set_widget_visible();

        let option_label = |_: &World, text: String, frame: HandleAny| {
            self::option_label(world, text, frame, camera)
        };
        let option_desc = |_: &World, text: String, frame: HandleAny| {
            self::option_desc(world, text, frame, camera)
        };
        let option_slider = |_: &World, frame: HandleAny| self::option_slider(world, frame, camera);

        let label1_frame = world.insert(EchoWidget);
        let label1 = world.insert(Text {
            text: tr("settings.brush.title").into(),
            metrics: Metrics {
                font_size: 14.0,
                line_height: 18.0,
            },
            attrs: Attrs::new().weight(Weight::BOLD),
            color: theme.gray4,
            camera: Some(camera),
            ..Default::default()
        });
        world.insert(Transform {
            value: TransformValue::anchor((0.0, 0.0), Rectangle::new_extend(16, 0, 56, 18)),
            source: label1_frame.untyped(),
            target: label1.untyped(),
        });

        // Flow //
        let flow_frame = world.insert(EchoWidget);
        let flow_label = option_label(world, String::new(), flow_frame.untyped());
        let flow_desc = option_desc(world, String::new(), flow_frame.untyped());
        let (flow_slider, flow_slider_label) = option_slider(world, flow_frame.untyped());
        world.observer(flow_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            match layer.active_mut().param_mut(BrushParamKey::Flow) {
                Some(BrushValueMut::Scalar(param)) => param.scale = value,
                Some(BrushValueMut::Vec4(flow)) => flow.w = value,
                _ => {}
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Blur Kernel Sigma //
        let sigma_frame = world.insert(EchoWidget);
        let sigma_label = option_label(world, String::new(), sigma_frame.untyped());
        let sigma_desc = option_desc(world, String::new(), sigma_frame.untyped());
        let (sigma_slider, sigma_slider_label) = option_slider(world, sigma_frame.untyped());
        world.observer(sigma_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::Sigma) {
                param.scale = value * 3.0;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Softness //
        let softness_frame = world.insert(EchoWidget);
        let softness_label = option_label(world, String::new(), softness_frame.untyped());
        let softness_desc = option_desc(world, String::new(), softness_frame.untyped());
        let (softness_slider, softness_slider_label) =
            option_slider(world, softness_frame.untyped());
        world.observer(softness_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::Softness) {
                param.scale = 1. - value;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Spacing //
        let spacing_frame = world.insert(EchoWidget);
        let spacing_label = option_label(world, String::new(), spacing_frame.untyped());
        let spacing_desc = option_desc(world, String::new(), spacing_frame.untyped());
        let (spacing_slider, spacing_slider_label) = option_slider(world, spacing_frame.untyped());
        world.observer(spacing_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::Spacing) {
                param.scale = value;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Color Ratio //
        let color_ratio_frame = world.insert(EchoWidget);
        let color_ratio_label = option_label(world, String::new(), color_ratio_frame.untyped());
        let color_ratio_desc = option_desc(world, String::new(), color_ratio_frame.untyped());
        let (color_ratio_slider, color_ratio_slider_label) =
            option_slider(world, color_ratio_frame.untyped());
        world.observer(color_ratio_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::ColorRatio) {
                param.scale = value;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Sample Radius //
        let sample_radius_frame = world.insert(EchoWidget);
        let sample_radius_label = option_label(world, String::new(), sample_radius_frame.untyped());
        let sample_radius_desc = option_desc(world, String::new(), sample_radius_frame.untyped());
        let (sample_radius_slider, sample_radius_slider_label) =
            option_slider(world, sample_radius_frame.untyped());
        world.observer(sample_radius_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::SampleRadius) {
                param.scale = value;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        // Sample Rate //
        let sample_rate_frame = world.insert(EchoWidget);
        let sample_rate_label = option_label(world, String::new(), sample_rate_frame.untyped());
        let sample_rate_desc = option_desc(world, String::new(), sample_rate_frame.untyped());
        let (sample_rate_slider, sample_rate_slider_label) =
            option_slider(world, sample_rate_frame.untyped());
        world.observer(sample_rate_slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::SampleRate) {
                param.scale = value;
            }
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        world.dependency(container, this);
        world.dependency(label1_frame, this);
        world.dependency(label1, this);
        world.dependency(flow_frame, this);
        world.dependency(sigma_frame, this);
        world.dependency(softness_frame, this);
        world.dependency(spacing_frame, this);
        world.dependency(color_ratio_frame, this);
        world.dependency(sample_radius_frame, this);
        world.dependency(sample_rate_frame, this);

        let ob = world.observer(layer, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(layer);

            let mut flow_label = world.fetch_mut(flow_label);
            let mut flow_desc = world.fetch_mut(flow_desc);
            let mut sigma_label = world.fetch_mut(sigma_label);
            let mut sigma_desc = world.fetch_mut(sigma_desc);
            let mut softness_label = world.fetch_mut(softness_label);
            let mut softness_desc = world.fetch_mut(softness_desc);
            let mut spacing_label = world.fetch_mut(spacing_label);
            let mut spacing_desc = world.fetch_mut(spacing_desc);
            let mut color_ratio_label = world.fetch_mut(color_ratio_label);
            let mut color_ratio_desc = world.fetch_mut(color_ratio_desc);
            let mut sample_radius_label = world.fetch_mut(sample_radius_label);
            let mut sample_radius_desc = world.fetch_mut(sample_radius_desc);
            let mut sample_rate_label = world.fetch_mut(sample_rate_label);
            let mut sample_rate_desc = world.fetch_mut(sample_rate_desc);

            // Flow //
            flow_label.set_text(tr("settings.brush.flow.label"));
            flow_desc.set_text(tr("settings.brush.flow.desc"));
            let flow = match layer.active().param(BrushParamKey::Flow) {
                Some(BrushValue::Scalar(param)) => param.scale,
                Some(BrushValue::Vec4(flow)) => flow.w,
                _ => 0.0,
            };
            world.queue_trigger(flow_slider, SetSliderValue(flow));
            world.queue_trigger(flow_slider_label, SetText(format!("{flow:.2}")));

            // Sigma //
            sigma_label.set_text(tr("settings.brush.sigma.label"));
            sigma_desc.set_text(tr("settings.brush.sigma.desc"));
            let sigma = layer
                .active()
                .scalar(BrushParamKey::Sigma)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            world.queue_trigger(sigma_slider, SetSliderValue(sigma / 3.0));
            world.queue_trigger(sigma_slider_label, SetText(format!("{sigma:.2}")));

            // Softness
            softness_label.set_text(tr("settings.brush.softness.label"));
            softness_desc.set_text(tr("settings.brush.softness.desc"));
            let softness = layer
                .active()
                .scalar(BrushParamKey::Softness)
                .map(|param| 1. - param.scale)
                .unwrap_or(1.0);
            world.queue_trigger(softness_slider, SetSliderValue(softness));
            world.queue_trigger(softness_slider_label, SetText(format!("{softness:.2}")));

            // Spacing //
            spacing_label.set_text(tr("settings.brush.spacing.label"));
            spacing_desc.set_text(tr("settings.brush.spacing.desc"));
            let spacing = layer
                .active()
                .scalar(BrushParamKey::Spacing)
                .map(|param| param.scale)
                .unwrap_or(0.1);
            world.queue_trigger(spacing_slider, SetSliderValue(spacing));
            world.queue_trigger(spacing_slider_label, SetText(format!("{spacing:.2}")));

            // Color Ratio //
            color_ratio_label.set_text(tr("settings.brush.color_ratio.label"));
            color_ratio_desc.set_text(tr("settings.brush.color_ratio.desc"));
            let color_ratio = layer
                .active()
                .scalar(BrushParamKey::ColorRatio)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            world.queue_trigger(color_ratio_slider, SetSliderValue(color_ratio));
            world.queue_trigger(
                color_ratio_slider_label,
                SetText(format!("{color_ratio:.2}")),
            );

            // Sample Radius //
            sample_radius_label.set_text(tr("settings.brush.sample_radius.label"));
            sample_radius_desc.set_text(tr("settings.brush.sample_radius.desc"));
            let sample_radius = layer
                .active()
                .scalar(BrushParamKey::SampleRadius)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            world.queue_trigger(sample_radius_slider, SetSliderValue(sample_radius));
            world.queue_trigger(
                sample_radius_slider_label,
                SetText(format!("{sample_radius:.2}")),
            );

            // Sample Rate //
            sample_rate_label.set_text(tr("settings.brush.sample_rate.label"));
            sample_rate_desc.set_text(tr("settings.brush.sample_rate.desc"));
            let sample_rate = layer
                .active()
                .scalar(BrushParamKey::SampleRate)
                .map(|param| param.scale)
                .unwrap_or(0.0);
            world.queue_trigger(sample_rate_slider, SetSliderValue(sample_rate));
            world.queue_trigger(
                sample_rate_slider_label,
                SetText(format!("{sample_rate:.2}")),
            );
        });
        world.dependency(ob, this);

        world.insert(LuniFlex {
            parent: (
                container.untyped(),
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
                (
                    spacing_frame.untyped(),
                    LuniChild {
                        basis: Some(108),
                        ..Default::default()
                    },
                ),
                (
                    color_ratio_frame.untyped(),
                    LuniChild {
                        basis: Some(108),
                        ..Default::default()
                    },
                ),
                (
                    sample_radius_frame.untyped(),
                    LuniChild {
                        basis: Some(108),
                        ..Default::default()
                    },
                ),
                (
                    sample_rate_frame.untyped(),
                    LuniChild {
                        basis: Some(108),
                        ..Default::default()
                    },
                ),
            ],
        });
    }
}

fn option_label(
    world: &World,
    text: String,
    frame: HandleAny,
    camera: Handle<Camera>,
) -> Handle<Text> {
    let theme = world.single_fetch::<Theme>();

    let label = world.insert(Text {
        text,
        metrics: Metrics {
            font_size: 16.0,
            line_height: 20.0,
        },
        color: theme.distinct,
        camera: Some(camera),
        ..Default::default()
    });

    world.dependency(label, frame);

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
        source: frame,
        target: label.untyped(),
    });

    label
}

fn option_desc(
    world: &World,
    text: String,
    frame: HandleAny,
    camera: Handle<Camera>,
) -> Handle<Text> {
    let theme = world.single_fetch::<Theme>();

    let label = world.insert(Text {
        text,
        metrics: Metrics {
            font_size: 12.0,
            line_height: 14.0,
        },
        color: theme.gray4,
        camera: Some(camera),
        ..Default::default()
    });

    world.dependency(label, frame);

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
        source: frame,
        target: label.untyped(),
    });

    label
}

fn option_slider(
    world: &World,
    frame: HandleAny,
    camera: Handle<Camera>,
) -> (Handle<Slider>, Handle<SliderLabel>) {
    let slider = world.insert(Slider {
        common: WidgetCommon {
            camera,
            visible: true,
        },
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
        source: frame,
        target: slider.untyped(),
    });

    let slider_label = world.insert(SliderLabel {
        text: String::new(),
        clockwise: false,
        source: slider,
        hover: false,
        visible: true,
        camera,
    });

    world.dependency(slider, frame);
    world.dependency(slider_label, frame);

    (slider, slider_label)
}
