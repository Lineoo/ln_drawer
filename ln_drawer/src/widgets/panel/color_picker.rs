use std::sync::Arc;

use glam::{IVec2, UVec2, Vec2};
use ln_world::{Handle, World};
use palette::{Hsla, IntoColor, Oklab, RgbHue, Srgba};

use crate::{
    layer::wrapper::{BrushConfigurationChanged, LayerWrapper},
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        palette::{
            hsl::{ColorHsla, HslPanel, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        panel::Panel,
        renderer::svg::svg_render,
        tabs::Tabs,
    },
};

pub fn color_picker_panel(world: &World, toggle_button: Handle<ToggleButton>) {
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

    super::settings::panel_settings(world, tab_settings);

    let tab_debug = world.insert(Panel {
        rect: Rectangle::default(),
        visible: true,
        shadow: false,
    });

    super::debug_panel::debug_panel(world, tab_debug);

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
            (
                ButtonImage {
                    transform: TransformValue::anchor(
                        (0.5, 0.5),
                        Rectangle::new_half(IVec2::ZERO, UVec2::splat(12)),
                    ),
                    bytes: Arc::new(image::DynamicImage::from(svg_render(
                        include_bytes!("../../../res/interface/bug.svg"),
                        1.0,
                    ))),
                },
                tab_debug.untyped(),
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
        layer.tint_brush.color = color.into_color();
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
        layer.tint_brush.color = color.into_color();
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(bar, move |&ColorOklab(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.round_brush.color = color.into_color();
        layer.tint_brush.color = color.into_color();
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let oklab = layer.round_brush.color.into_color();
        world.trigger(polar, &SetColorOklab(oklab));
        world.trigger(bar, &SetColorOklab(oklab));
    });
}
