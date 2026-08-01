use std::sync::Arc;

use glam::{IVec2, UVec2, Vec2};
use ln_world::{Descriptor, Handle, World};
use palette::{Hsla, IntoColor, RgbHue, Srgba};

use crate::{
    layer::wrapper::LayerWrapper,
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        palette::hsl::{PaletteHsl, PaletteHsla},
        panel::Panel,
        renderer::svg::svg_render,
        tabs::Tabs,
    },
};

pub struct ColorPicker(pub Handle<ToggleButton>);
impl Descriptor for ColorPicker {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let color_picker = self.0;

        let color_picker_color = world.build(RoundedRectDescriptor {
            order: 21,
            color: Srgba::new(0.9, 0.7, 0.7, 1.0),
            value: 10.0,
            shrink: 10.0,
            shadow_offset: Vec2::ZERO,
            vertex_extend: 20,
            ..Default::default()
        });

        world.observer(color_picker, move |&SetWidgetRectangle(rect), world| {
            let transform = Transform {
                value: TransformValue::anchor(
                    (0.5, 0.5),
                    Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
                ),
                source: color_picker.untyped(),
                target: color_picker_color.untyped(),
            };

            let target = transform.value.compute(rect);

            world.queue_trigger(color_picker_color, SetWidgetRectangle(target));
        });

        let tab01 = world.build(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        let tab02 = world.build(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: false,
        });

        let palette_hsl = world.build(PaletteHsl {
            rect: Rectangle::default(),
            color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
            enabled: true,
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
            ),
            source: tab01.untyped(),
            target: palette_hsl.untyped(),
        });

        world.observer(palette_hsl, move |&PaletteHsla(color), world| {
            let mut layer = world.single_fetch_mut::<LayerWrapper>().unwrap();
            let mut color_picker_color = world.fetch_mut(color_picker_color).unwrap();
            layer.brush.modifier.color = color.into_color();
            color_picker_color.desc.color = color.into_color();
        });

        let tabs = world.build(Tabs {
            active: 0,
            rect: Rectangle::default(),
            visible: false,
            tabs: vec![
                (
                    ButtonImage {
                        transform: TransformValue::anchor(
                            (0.5, 0.5),
                            Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
                        ),
                        bytes: Arc::new(image::DynamicImage::from(svg_render(
                            include_bytes!("../../../res/interface/palette.svg"),
                            1.0,
                        ))),
                    },
                    tab01.untyped(),
                ),
                (
                    ButtonImage {
                        transform: TransformValue::anchor(
                            (0.5, 0.5),
                            Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
                        ),
                        bytes: Arc::new(image::DynamicImage::from(svg_render(
                            include_bytes!("../../../res/interface/settings.svg"),
                            1.0,
                        ))),
                    },
                    tab02.untyped(),
                ),
            ],
        });

        world.observer(color_picker, move |&SetWidgetRectangle(rect), world| {
            let transform = TransformValue::anchor(
                (1.0, 0.5),
                Rectangle::new_half(IVec2::new(144 + 20, 0), UVec2::splat(144)),
            );
            let rect = transform.compute(rect);
            world.queue_trigger(tabs, SetWidgetRectangle(rect));
        });

        world.observer(color_picker, move |&ButtonSelected(selected), world| {
            world.queue_trigger(color_picker, SetButtonSelected(selected));
            world.queue_trigger(tabs, SetWidgetVisible(selected));
        });
    }
}
