use std::sync::Arc;

use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, HandleGeneric, World};
use palette::{Hsla, IntoColor, Oklab, RgbHue, Srgba};

use crate::{
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::camera::Camera,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{Container, ContainerDescriptor},
        echo::Echo,
        palette::{
            hsl::{ColorHsla, HslPanel, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        renderer::{
            rrect::{RRect, SetRRectColor},
            svg::svg_render,
        },
        tabs::Tabs,
    },
};

const PANEL_WIDTH: i32 = 384;
const PANEL_HEIGHT: i32 = 320;

pub fn color_picker_panel(
    world: &World,
    toggle_button: Handle<ToggleButton>,
    camera: Handle<Camera>,
) {
    let toggle_button_color_icon = world.insert(RRect {
        rect: Rectangle::default(),
        order: 21,
        color: Srgba::new(0.9, 0.7, 0.7, 1.0),
        radius: 10.0,
        width: 0.0,
        enabled: true,
        camera,
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

    let (tab_palette_hsl, cam_palette_hsl) = world.build(ContainerDescriptor {
        parent: camera,
        rect: Rectangle::default(),
        inner_transform: TransformValue::copy(),
        visible: false,
    });

    let (tab_palette_oklch, cam_palette_oklch) = world.build(ContainerDescriptor {
        parent: camera,
        rect: Rectangle::default(),
        inner_transform: TransformValue::copy(),
        visible: false,
    });

    let (tab_layer_selection, cam_layer_selection) = world.build(ContainerDescriptor {
        parent: camera,
        rect: Rectangle::default(),
        inner_transform: TransformValue::copy(),
        visible: false,
    });

    let (tab_debug, cam_debug) = world.build(ContainerDescriptor {
        parent: camera,
        rect: Rectangle::default(),
        inner_transform: TransformValue::copy(),
        visible: false,
    });

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
                        include_bytes!("../../../res/interface/layers.svg"),
                        1.0,
                    ))),
                },
                tab_layer_selection.untyped(),
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
        camera,
    });

    let layer = world.single::<LayerPage>().unwrap();
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let color = layer.color();
        world.queue_trigger(toggle_button_color_icon, SetRRectColor(color.into_color()));
        // trigger palette_hsl SetPaletteHsl
    });

    world.observer(toggle_button, move |&SetWidgetRectangle(rect), world| {
        let transform = TransformValue::anchor(
            (1.0, 1.0),
            Rectangle::new_extend(20, -PANEL_HEIGHT, PANEL_WIDTH as u32, PANEL_HEIGHT as u32),
        );
        let rect = transform.compute(rect);
        world.queue_trigger(tabs, SetWidgetRectangle(rect));
    });

    world.observer(toggle_button, move |&ButtonSelected(selected), world| {
        world.queue_trigger(toggle_button, SetButtonSelected(selected));
    });

    world.observer(toggle_button, move |&SetButtonSelected(selected), world| {
        world.queue_trigger(tabs, SetWidgetVisible(selected));
    });

    world.dependency(toggle_button_color_icon, toggle_button);
    world.dependency(tab_palette_hsl, toggle_button);
    world.dependency(tab_palette_oklch, toggle_button);
    world.dependency(tab_layer_selection, toggle_button);
    world.dependency(tab_debug, toggle_button);
    world.dependency(tabs, toggle_button);

    world.named("palette_hsl");
    palette_hsl(world, tab_palette_hsl, cam_palette_hsl);
    world.named("palette_oklab");
    palette_oklab(world, tab_palette_oklch, cam_palette_oklch);
    world.named("layer_selection");
    super::layer_selection::layer_selection(world, tab_layer_selection, cam_layer_selection);
    world.named("debug_panel");
    super::debug_panel::debug_panel(world, tab_debug, cam_debug);
}

fn palette_hsl(world: &World, bg: Handle<Container>, camera: Handle<Camera>) {
    let panel = world.insert(HslPanel {
        rect: Rectangle::default(),
        color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
        enabled: false,
        camera,
    });

    world.dependency(panel, bg);

    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
        ),
        source: bg.untyped(),
        target: panel.untyped(),
    });

    let layer = world.single::<LayerPage>().unwrap();
    world.observer(panel, move |&ColorHsla(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.set_color(color.into_color());
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let hsla = layer.color().into_color();
        world.trigger(panel, &SetColorHsla(hsla));
    });
}

fn palette_oklab(world: &World, bg: Handle<Container>, camera: Handle<Camera>) {
    let polar = world.insert(OklabPolar {
        rect: Rectangle::default(),
        color: Oklab::default(),
        enabled: false,
        camera,
    });
    let bar = world.insert(OklabBar {
        rect: Rectangle::default(),
        color: Oklab::default(),
        enabled: false,
        camera,
    });

    world.dependency(polar, bg);
    world.dependency(bar, bg);

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

    let layer = world.single::<LayerPage>().unwrap();
    world.observer(polar, move |&ColorOklab(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.set_color(color.into_color());
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(bar, move |&ColorOklab(color), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.set_color(color.into_color());
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer).unwrap();
        let oklab = layer.color().into_color();
        world.trigger(polar, &SetColorOklab(oklab));
        world.trigger(bar, &SetColorOklab(oklab));
    });
}

pub struct LayerOklabPalette {
    pub common: WidgetCommon,
}

impl Element for LayerOklabPalette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let polar = world.insert(OklabPolar {
            rect: Rectangle::default(),
            color: Oklab::default(),
            enabled: self.common.visible,
            camera: self.common.camera,
        });
        let bar = world.insert(OklabBar {
            rect: Rectangle::default(),
            color: Oklab::default(),
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        world.dependency(polar, this);
        world.dependency(bar, this);

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(-30, 0), UVec2::splat(100)),
            ),
            source: this.untyped(),
            target: polar.untyped(),
        });
        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(110, 0), UVec2::new(20, 100)),
            ),
            source: this.untyped(),
            target: bar.untyped(),
        });

        let layer = world.single::<LayerPage>().unwrap();
        world.observer(polar, move |&ColorOklab(color), world| {
            let mut layer = world.fetch_mut(layer).unwrap();
            layer.set_color(color.into_color());
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(bar, move |&ColorOklab(color), world| {
            let mut layer = world.fetch_mut(layer).unwrap();
            layer.set_color(color.into_color());
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(layer).unwrap();
            let oklab = layer.color().into_color();
            world.trigger(polar, &SetColorOklab(oklab));
            world.trigger(bar, &SetColorOklab(oklab));
        });

        Echo::new(world, this).widget_rectangle().widget_visible();
    }
}
