use std::sync::Arc;

use glam::{IVec2, UVec2, Vec2};
use ln_world::{Descriptor, Handle, World};
use palette::{Hsla, IntoColor, RgbHue, Srgba};

use crate::{
    layer::wrapper::LayerWrapper,
    layout::{
        luni::{
            LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex, LuniParent,
            LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::Rectangle,
    render::rounded::{RoundedRect, RoundedRectDescriptor},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        echo::EchoWidget,
        palette::hsl::{PaletteHsl, PaletteHsla},
        panel::Panel,
        renderer::{svg::svg_render, text::Text},
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

        let tab_palette = world.insert(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        palette(world, tab_palette, toggle_button_color_icon);

        let tab_settings = world.insert(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        settings(world, tab_settings);

        let tabs = world.build(Tabs {
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
                    tab_palette.untyped(),
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

        world.observer(toggle_button, move |&SetWidgetRectangle(rect), world| {
            let transform = TransformValue::anchor(
                (1.0, 0.5),
                Rectangle::new_half(IVec2::new(144 + 20, 0), UVec2::splat(144)),
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

fn palette(world: &World, panel: Handle<Panel>, toggle_button_color_icon: Handle<RoundedRect>) {
    let palette_hsl = world.insert(PaletteHsl {
        rect: Rectangle::default(),
        color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
        enabled: true,
    });

    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
        ),
        source: panel.untyped(),
        target: palette_hsl.untyped(),
    });

    world.observer(palette_hsl, move |&PaletteHsla(color), world| {
        let mut layer = world.single_fetch_mut::<LayerWrapper>().unwrap();
        let mut toggle_button_color_icon = world.fetch_mut(toggle_button_color_icon).unwrap();
        layer.brush.modifier.color = color.into_color();
        toggle_button_color_icon.desc.color = color.into_color();
    });
}

fn settings(world: &World, panel: Handle<Panel>) {
    let label1_frame = world.insert(EchoWidget);
    let label1 = world.insert(Text {
        text: String::from("选项标签"),
        rect: Rectangle::new(0, 0, 56, 18),
        metrics: cosmic_text::Metrics {
            font_size: 14.0,
            line_height: 18.0,
        },
        ..Default::default()
    });
    world.insert(Transform {
        value: TransformValue::anchor((0.0, 0.0), Rectangle::new_extend(16, 0, 56, 18)),
        source: label1_frame.untyped(),
        target: label1.untyped(),
    });

    let option1_frame = world.insert(EchoWidget);
    let option1 = world.insert(Text {
        text: String::from("选项设置文本"),
        rect: Rectangle::new(0, 0, 144, 36),
        metrics: cosmic_text::Metrics {
            font_size: 14.0,
            line_height: 18.0,
        },
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
                offset: 0,
            },
        },
        source: option1_frame.untyped(),
        target: option1.untyped(),
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
                option1_frame.untyped(),
                LuniChild {
                    basis: Some(72),
                    ..Default::default()
                },
            ),
        ],
    });
}
