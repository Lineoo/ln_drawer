use glam::{IVec2, UVec2, Vec2};
use ln_world::{Descriptor, Handle, World};
use palette::{Hsla, IntoColor, RgbHue, Srgba};

use crate::{
    layer::wrapper::LayerWrapper,
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    widgets::{
        WidgetClick, WidgetEnabled, WidgetHsla,
        button::{Button, ButtonAnim, ButtonColor},
        palette::hsl::PaletteHsl,
    },
};

pub struct ColorPicker(pub Handle<Button>);
impl Descriptor for ColorPicker {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let color_picker = self.0;

        let color_picker_color = world.insert(Button {
            order: 11,
            color: Srgba::new(0.9, 0.7, 0.7, 1.0),
            attach_pointer: false,
            roundness: 10.0,
            shadow_offset: Vec2::ZERO,
            ..Default::default()
        });

        let main_panel_transform = TransformValue::anchor(
            (1.0, 0.5),
            Rectangle::new_half(IVec2::new(144 + 20, 0), UVec2::splat(144)),
        );

        let main_panel_transform_start = TransformValue::anchor(
            (1.0, 0.5),
            Rectangle::new_half(IVec2::new(144 / 4 + 20, 0), UVec2::splat(144 / 4)),
        );

        let palette_transform = TransformValue::scale(0.8, 0.8);

        let main_panel = world.insert(Button {
            attach_pointer: false,
            order: 0,
            enabled: false,
            ..Default::default()
        });

        let palette = world.insert(PaletteHsl {
            rect: Rectangle::default(),
            color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
            enabled: false,
        });

        world.dependency(palette, main_panel);

        world.insert(Transform {
            value: main_panel_transform,
            source: color_picker.untyped(),
            target: main_panel.untyped(),
        });

        world.insert(Transform {
            value: palette_transform,
            source: main_panel.untyped(),
            target: palette.untyped(),
        });

        world.observer(palette, move |&WidgetHsla(color), world| {
            let mut layer = world.single_fetch_mut::<LayerWrapper>().unwrap();
            layer.brush.modifier.color = color.into_color();
            world.queue_trigger(color_picker_color, ButtonColor(color.into_color()));
        });

        world.observer(color_picker, move |&WidgetClick, world| {
            let main_panel = world.fetch(main_panel).unwrap();
            let child2 = world.fetch(color_picker).unwrap();
            world.queue_trigger(main_panel.handle(), WidgetEnabled(!main_panel.enabled));
            world.queue_trigger(palette, WidgetEnabled(!main_panel.enabled));

            if !main_panel.enabled {
                world.queue_trigger(
                    main_panel.handle(),
                    ButtonAnim {
                        src: main_panel_transform_start.compute(child2.rect),
                        dst: main_panel_transform.compute(child2.rect),
                        hidden_after_finished: false,
                    },
                );
            }
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
            ),
            source: color_picker.untyped(),
            target: color_picker_color.untyped(),
        });
    }
}
