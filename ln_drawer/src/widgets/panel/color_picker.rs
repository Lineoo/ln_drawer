use glam::{I64Vec2, IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    color::{
        hsl::{hsl_to_srgb, srgb_to_hsl},
        oklab::{linear_srgb_to_oklab, oklab_to_linear_srgb},
        srgb::{mul_alpha, srgb_gamma_decode, srgb_gamma_encode},
    },
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::camera::Camera,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{Container, ContainerWithCamera},
        echo::Echo,
        palette::{
            hsl::{ColorHsla, HslPanel, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        panel::debug_panel::DebugPanel,
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
        color: Vec4::new(0.9, 0.7, 0.7, 1.0),
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

    let (tab_palette_hsl, cam_palette_hsl) = world.build(ContainerWithCamera {
        common: WidgetCommon {
            camera,
            visible: false,
        },
        rect: Rectangle::default(),
        inner: TransformValue::copy(),
        scroll: I64Vec2::ZERO,
    });

    world.named("palette_oklab");
    let tab_palette_oklab = world.insert(LayerOklabPalette {
        common: WidgetCommon {
            camera,
            visible: false,
        },
        rect: Rectangle::default(),
        oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
    });

    let (tab_layer_selection, cam_layer_selection) = world.build(ContainerWithCamera {
        rect: Rectangle::default(),
        inner: TransformValue::copy(),
        common: WidgetCommon {
            camera,
            visible: false,
        },
        scroll: I64Vec2::ZERO,
    });

    world.named("debug_panel");
    let tab_debug = world.insert(DebugPanel {
        common: WidgetCommon {
            camera,
            visible: false,
        },
        rect: Rectangle::default(),
    });

    let tabs = world.insert(Tabs {
        active: 0,
        rect: Rectangle::default(),
        visible: false,
        tabs: vec![
            (
                svg_render(include_bytes!("../../../res/interface/palette.svg"), 12),
                tab_palette_hsl.untyped(),
            ),
            (
                svg_render(include_bytes!("../../../res/interface/palette.svg"), 12),
                tab_palette_oklab.untyped(),
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
        camera,
    });

    let layer = world.single::<LayerPage>();
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer);
        // S(L) -> P(L)
        let color = mul_alpha(srgb_gamma_decode(layer.color()));
        world.queue_trigger(toggle_button_color_icon, SetRRectColor(color));
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
    world.dependency(tab_palette_oklab, toggle_button);
    world.dependency(tab_layer_selection, toggle_button);
    world.dependency(tab_debug, toggle_button);
    world.dependency(tabs, toggle_button);

    world.named("palette_hsl");
    palette_hsl(world, tab_palette_hsl, cam_palette_hsl);

    world.named("layer_selection");
    super::layer_selection::layer_selection(world, tab_layer_selection, cam_layer_selection);
}

fn palette_hsl(world: &World, bg: Handle<Container>, camera: Handle<Camera>) {
    let panel = world.insert(HslPanel {
        rect: Rectangle::default(),
        hsl: Vec4::new(0.3, 0.5, 0.5, 1.0),
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

    let layer = world.single::<LayerPage>();
    world.observer(panel, move |&ColorHsla(hsla), world| {
        let mut layer = world.fetch_mut(layer);
        layer.set_color(hsl_to_srgb(hsla));
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });
    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let layer = world.fetch(layer);
        let hsla = srgb_to_hsl(layer.color());
        world.trigger(panel, &SetColorHsla(hsla));
    });
}

pub struct LayerOklabPalette {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub oklab: Vec4,
}

impl Element for LayerOklabPalette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            inner: TransformValue::copy(),
            scroll: I64Vec2::ZERO,
        });

        let polar = world.insert(OklabPolar {
            rect: Rectangle::default(),
            oklab: self.oklab,
            enabled: self.common.visible,
            camera,
        });

        let bar = world.insert(OklabBar {
            rect: Rectangle::default(),
            oklab: self.oklab,
            enabled: self.common.visible,
            camera,
        });

        Echo::new(world, this).widget_rectangle().widget_visible();

        world.insert(Transform {
            value: TransformValue::copy(),
            source: this.untyped(),
            target: container.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(-30, 0), UVec2::splat(100)),
            ),
            source: container.untyped(),
            target: polar.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(110, 0), UVec2::new(20, 100)),
            ),
            source: container.untyped(),
            target: bar.untyped(),
        });

        let layer = world.single::<LayerPage>();
        world.observer(polar, move |&ColorOklab(color), world| {
            let mut layer = world.fetch_mut(layer);
            layer.set_color(srgb_gamma_encode(oklab_to_linear_srgb(color)));
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(bar, move |&ColorOklab(color), world| {
            let mut layer = world.fetch_mut(layer);
            layer.set_color(srgb_gamma_encode(oklab_to_linear_srgb(color)));
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(layer);
            let oklab = linear_srgb_to_oklab(srgb_gamma_decode(layer.color()));
            world.trigger(polar, &SetColorOklab(oklab));
            world.trigger(bar, &SetColorOklab(oklab));
        });

        world.dependency(container, this);
        world.dependency(camera, this);
        world.dependency(polar, this);
        world.dependency(bar, this);
    }
}
