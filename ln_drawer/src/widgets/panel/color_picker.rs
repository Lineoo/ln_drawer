use std::f32::consts::SQRT_2;

use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, World};

use crate::{
    color::{
        hsl::{hsl_to_srgb, srgb_to_hsl},
        oklab::{linear_srgb_to_oklab, oklab_to_linear_srgb},
        srgb::{mul_alpha, srgb_gamma_decode, srgb_gamma_encode},
    },
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::{
        transform::{Transform, TransformValue},
        val::{Val, ValChanged},
    },
    measures::{FI64Ext, Rectangle},
    render::camera::PageCamera,
    theme::Theme,
    widgets::{
        button::{
            ButtonDrag, ButtonDragStatus, ButtonImage, ButtonSelected, SetButtonSelected,
            ToggleButton,
        },
        common::WidgetCommon,
        container::{
            ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerEcho, ContainerWithCamera,
        },
        object::color_snip::{ColorSnip, ColorSnipAttr},
        palette::{
            hsl::{ColorHsla, HslRing, HslSquare, INNER_RADIUS, SetColorHsla},
            oklab::{ColorOklab, OklabBar, OklabPolar, SetColorOklab},
        },
        panel::{debug_panel::DebugPanel, layer_selection::LayerSelectionPanel},
        renderer::{rrect::RRect, svg::svg_render},
        tabs::{TabEntry, Tabs},
    },
};

const PANEL_WIDTH: i32 = 384;
const PANEL_HEIGHT: i32 = 320;

pub struct ColorPickerPanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for ColorPickerPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("color_picker_panel");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let hsl_rect = world.insert(Val::new(Rectangle::default()));
        let hsl_visible = world.insert(Val::new(false));
        world.insert(LayerHslPalette {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: hsl_visible,
            },
            rect: hsl_rect,
            hsl: Vec4::new(0.3, 0.5, 0.5, 1.0),
        });

        let oklab_rect = world.insert(Val::new(Rectangle::default()));
        let oklab_visible = world.insert(Val::new(false));
        world.insert(LayerOklabPalette {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: oklab_visible,
            },
            rect: oklab_rect,
            oklab: Vec4::new(0.0, 0.0, 0.0, 1.0),
        });

        let layer_rect = world.insert(Val::new(Rectangle::default()));
        let layer_visible = world.insert(Val::new(false));
        world.insert(LayerSelectionPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: layer_visible,
            },
            rect: layer_rect,
        });

        let debug_rect = world.insert(Val::new(Rectangle::default()));
        let debug_visible = world.insert(Val::new(false));
        world.insert(DebugPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: debug_visible,
            },
            rect: debug_rect,
        });

        world.insert(Tabs {
            common: self.common,
            rect: self.rect,
            // TODO TabItem like SliderLabel
            tabs: vec![
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/palette.svg")),
                        12,
                    ),
                    rect: hsl_rect,
                    visible: hsl_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/palette.svg")),
                        12,
                    ),
                    rect: oklab_rect,
                    visible: oklab_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/layers.svg")),
                        12,
                    ),
                    rect: layer_rect,
                    visible: layer_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/bug.svg")),
                        12,
                    ),
                    rect: debug_rect,
                    visible: debug_visible,
                },
            ],
            active: 0,
        });
    }
}

pub struct ColorPickerButton {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for ColorPickerButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("color_picker_button");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let theme = world.single_fetch::<Theme>();
        let page = world.single_fetch::<LayerPage>();
        let button = world.insert(ToggleButton {
            common: self.common,
            rect: self.rect,
            theme: theme.button(),
            image: None,
            selected: false,
            hovering: false,
        });

        let icon_transform = TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
        );
        let icon_rect = world.build(Transform::new(icon_transform, self.rect));
        let color = world.insert(Val::new(mul_alpha(srgb_gamma_decode(page.color()))));

        world.insert(RRect {
            common: self.common,
            rect: icon_rect,
            order: 21,
            color,
            radius: 10.0,
            width: 0.0,
        });

        let panel_rect = world.build(Transform::anchor(
            (1.0, 1.0),
            Rectangle::new_extend(20, -PANEL_HEIGHT, PANEL_WIDTH as u32, PANEL_HEIGHT as u32),
            self.rect,
        ));
        let visible = world.insert(Val::new(false));

        world.insert(ColorPickerPanel {
            common: WidgetCommon {
                visible,
                ..self.common
            },
            rect: panel_rect,
        });

        world.observer(button, move |event: &ButtonSelected, world| {
            world.queue_trigger(this, ButtonSelected(event.0));
        });

        world.observer(this, move |&SetButtonSelected(selected), world| {
            world.queue_trigger(button, SetButtonSelected(selected));
            let mut visible = world.fetch_mut(visible);
            visible.val = selected;
        });

        world.observer(page.handle(), move |&BrushConfigurationChanged, world| {
            let layer = world.single_fetch::<LayerPage>();
            let next = mul_alpha(srgb_gamma_decode(layer.color()));
            let mut current = world.fetch_mut(color);
            current.val = next;
            world.queue_trigger(color, ValChanged);
        });

        let mut dragging = None;
        world.observer(button, move |drag: &ButtonDrag, world| {
            let this = world.fetch(this);
            let page_camera = world.single_fetch::<PageCamera>().0;
            let page_camera = world.fetch(page_camera);
            let page = world.single_fetch::<LayerPage>();
            let snip_rect = *dragging.get_or_insert_with(|| {
                let position = page_camera
                    .screen_to_world(drag.here.pointer.screen)
                    .q32_round();
                let rect = world.insert(Val::new(Rectangle::new_half(position, UVec2::splat(24))));

                world.insert(ColorSnip {
                    common: WidgetCommon {
                        camera: page_camera.handle(),
                        visible: this.common.visible,
                    },
                    rect,
                    attr: ColorSnipAttr {
                        position,
                        color: page.color(),
                    },
                });

                rect
            });

            let next = Rectangle::new_half(
                page_camera
                    .screen_to_world(drag.here.pointer.screen)
                    .q32_round(),
                UVec2::splat(24),
            );
            world.queue(move |world| {
                let mut rect = world.fetch_mut(snip_rect);
                rect.val = next;
                world.queue_trigger(rect.handle(), ValChanged);
            });

            if drag.status == ButtonDragStatus::End {
                dragging = None;
            }
        });
    }
}

pub struct LayerHslPalette {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub hsl: Vec4,
}

impl Element for LayerHslPalette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        const RING_HALF: u32 = 100;
        const SQUARE_HALF: u32 = (RING_HALF as f32 * INNER_RADIUS * SQRT_2) as u32;

        world.scope("palette_hsl");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let restriction = world.build(ContainerDecoDropdown {
            height: None,
            outer: self.rect,
        });
        world.insert(ContainerDeco {
            container,
            restriction,
        });

        let ring_rect = world.build(Transform::new(
            TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(RING_HALF)),
            ),
            restriction,
        ));
        let ring = world.insert(HslRing {
            common,
            rect: ring_rect,
            hsl: self.hsl,
            inner_radius: INNER_RADIUS,
        });

        let square_rect = world.build(Transform::new(
            TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(SQUARE_HALF)),
            ),
            restriction,
        ));
        let square = world.insert(HslSquare {
            common,
            rect: square_rect,
            hsl: self.hsl,
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
    }
}

pub struct LayerOklabPalette {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub oklab: Vec4,
}

impl Element for LayerOklabPalette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("palette_oklab");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let restriction = world.build(ContainerDecoDropdown {
            height: None,
            outer: self.rect,
        });
        world.insert(ContainerDeco {
            container,
            restriction,
        });
        let local = world.build(ContainerEcho { container });

        let polar_rect = world.build(Transform::new(
            TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(-30, 0), UVec2::splat(100)),
            ),
            local,
        ));
        let polar = world.insert(OklabPolar {
            common,
            rect: polar_rect,
            oklab: self.oklab,
        });

        let bar_rect = world.build(Transform::new(
            TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::new(110, 0), UVec2::new(20, 100)),
            ),
            local,
        ));
        let bar = world.insert(OklabBar {
            common,
            rect: bar_rect,
            oklab: self.oklab,
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
    }
}
