use std::f32::consts::SQRT_2;

use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    color::{
        hsl::{hsl_to_srgb, srgb_to_hsl},
        oklab::{linear_srgb_to_oklab, oklab_to_linear_srgb},
        srgb::{mul_alpha, srgb_gamma_decode, srgb_gamma_encode},
    },
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::{
        echo::Echo,
        transfer::Transfer,
        transform::{Transform, TransformValue},
    },
    measures::{FI64Ext, Rectangle},
    render::camera::PageCamera,
    theme::Theme,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonDrag, ButtonDragStatus, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerWithCamera},
        object::color_snip::{ColorSnip, ColorSnipAttr},
        palette::{
            hsl::{ColorHsla, HslRing, HslSquare, INNER_RADIUS, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        panel::{debug_panel::DebugPanel, layer_selection::LayerSelectionPanel},
        renderer::{
            rrect::{RRect, SetRRectColor},
            svg::svg_render,
        },
        tabs::Tabs,
    },
};

const PANEL_WIDTH: i32 = 384;
const PANEL_HEIGHT: i32 = 320;

pub struct ColorPickerPanel {
    pub common: WidgetCommon,
    pub button: Handle<ToggleButton>,
}

impl Element for ColorPickerPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.named("palette_hsl");
        let tab_palette_hsl = world.insert(LayerHslPalette {
            common: self.common,
            rect: Rectangle::default(),
            hsl: Vec4::new(0.3, 0.5, 0.5, 1.0),
        });

        world.named("palette_oklab");
        let tab_palette_oklab = world.insert(LayerOklabPalette {
            common: self.common,
            rect: Rectangle::default(),
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        world.named("layer_selection");
        let tab_layer_selection = world.insert(LayerSelectionPanel {
            common: self.common,
        });

        world.named("debug_panel");
        let tab_debug = world.insert(DebugPanel {
            common: self.common,
            rect: Rectangle::default(),
        });

        let tabs = world.insert(Tabs {
            active: 0,
            rect: Rectangle::default(),
            visible: self.common.visible,
            // TODO TabItem like SliderLabel
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
            camera: self.common.camera,
        });

        world.observer(self.button, move |&SetWidgetRectangle(rect), world| {
            let transform = TransformValue::anchor(
                (1.0, 1.0),
                Rectangle::new_extend(20, -PANEL_HEIGHT, PANEL_WIDTH as u32, PANEL_HEIGHT as u32),
            );
            let rect = transform.compute(rect);
            world.queue_trigger(tabs, SetWidgetRectangle(rect));
        });

        world.observer(self.button, move |&SetButtonSelected(selected), world| {
            world.queue_trigger(tabs, SetWidgetVisible(selected));
        });

        world.dependency(tab_palette_hsl, this);
        world.dependency(tab_palette_oklab, this);
        world.dependency(tab_layer_selection, this);
        world.dependency(tab_debug, this);
        world.dependency(tabs, this);
    }
}

pub struct ColorPickerButton {
    pub common: WidgetCommon,
    pub rect: Rectangle,
}

impl Element for ColorPickerButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let page = world.single::<LayerPage>();

        let button = world.insert(ToggleButton {
            common: self.common,
            rect: self.rect,
            theme: theme.button(),
            image: None,
            selected: false,
            hovering: false,
        });

        let button_color_icon = world.insert(RRect {
            rect: Rectangle::default(),
            order: 21,
            color: Vec4::new(0.9, 0.7, 0.7, 1.0),
            radius: 10.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let color_picker_panel = world.insert(ColorPickerPanel {
            common: WidgetCommon {
                visible: false,
                ..self.common
            },
            button,
        });

        Transfer::new(world, this, button)
            .set_widget_rectangle()
            .set_widget_visible()
            .set_button_selected();

        Transfer::new(world, button, this).button_selected();

        world.observer(page, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(page);
            let color = mul_alpha(srgb_gamma_decode(layer.color()));
            world.queue_trigger(button_color_icon, SetRRectColor(color));
        });

        world.observer(button, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            let target = TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
            )
            .compute(rect);
            world.queue_trigger(button_color_icon, SetWidgetRectangle(target));
        });

        world.observer(button, move |&SetButtonSelected(selected), world| {
            world.queue_trigger(color_picker_panel, SetWidgetVisible(selected));
        });

        let mut dragging = None;
        world.observer(button, move |drag: &ButtonDrag, world| {
            let page_camera = world.single_fetch::<PageCamera>().0;
            let page_camera = world.fetch(page_camera);
            let page = world.single_fetch::<LayerPage>();
            let snip = dragging.get_or_insert_with(|| {
                world.insert(ColorSnip {
                    common: WidgetCommon {
                        camera: page_camera.handle(),
                        visible: true,
                    },
                    attr: ColorSnipAttr {
                        position: page_camera
                            .screen_to_world(drag.here.pointer.screen)
                            .q32_round(),
                        color: page.color(),
                    },
                })
            });

            world.queue_trigger(
                *snip,
                SetWidgetRectangle(Rectangle::new_half(
                    page_camera
                        .screen_to_world(drag.here.pointer.screen)
                        .q32_round(),
                    UVec2::splat(24),
                )),
            );

            if drag.status == ButtonDragStatus::End {
                dragging = None;
            }
        });

        world.dependency(button, this);
        world.dependency(button_color_icon, this);
        world.dependency(color_picker_panel, this);
    }
}

pub struct LayerHslPalette {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub hsl: Vec4,
}

impl Element for LayerHslPalette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        const RING_HALF: u32 = 100;
        const SQUARE_HALF: u32 = (RING_HALF as f32 * INNER_RADIUS * SQRT_2) as u32;

        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
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

        let ring = world.insert(HslRing {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
            },
            rect: Rectangle::default(),
            hsl: self.hsl,
            inner_radius: INNER_RADIUS,
        });

        let square = world.insert(HslSquare {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
            },
            rect: Rectangle::default(),
            hsl: self.hsl,
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
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(RING_HALF)),
            ),
            source: container.untyped(),
            target: ring.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(SQUARE_HALF)),
            ),
            source: container.untyped(),
            target: square.untyped(),
        });

        let layer = world.single::<LayerPage>();
        world.observer(ring, move |&ColorHsla(hsla), world| {
            let mut layer = world.fetch_mut(layer);
            layer.set_color(hsl_to_srgb(hsla));
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(square, move |&ColorHsla(hsla), world| {
            let mut layer = world.fetch_mut(layer);
            layer.set_color(hsl_to_srgb(hsla));
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });
        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let layer = world.fetch(layer);
            let hsla = srgb_to_hsl(layer.color());
            world.trigger(ring, &SetColorHsla(hsla));
            world.trigger(square, &SetColorHsla(hsla));
        });

        world.dependency(container, this);
        world.dependency(camera, this);
        world.dependency(ring, this);
        world.dependency(square, this);
    }
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

        let polar = world.insert(OklabPolar {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
            },
            rect: Rectangle::default(),
            oklab: self.oklab,
        });

        let bar = world.insert(OklabBar {
            common: WidgetCommon {
                camera,
                visible: self.common.visible,
            },
            rect: Rectangle::default(),
            oklab: self.oklab,
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
