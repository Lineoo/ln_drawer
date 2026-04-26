use palette::{Hsla, IntoColor, RgbHue};

use crate::{
    layout::{
        LayoutEnableAction,
        transform::{Transform, TransformValue},
    },
    measures::{Position, Rectangle, Size},
    stroke::StrokeLayer,
    widgets::{
        WidgetExpanded, WidgetHsla, WidgetRectangle, expandable::Expandable,
        palette::hsl::PaletteHsl, translatable::Translatable,
    },
    world::{Element, Handle, World},
};

pub mod hsl;

pub struct ColorPicker;

impl ColorPicker {
    fn insert(world: &World) {
        let palette = world.insert(PaletteHsl {
            rect: Rectangle::new(100, 100, 300, 300),
            color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
            enabled: false,
        });

        let expandable = world.insert(Expandable {
            rect: Rectangle::new(-100, -100, -50, -50),
            transform: TransformValue::scale(10.0, 10.0),
            expanded: false,
        });

        let translatable = world.insert(Translatable {
            rect: Rectangle::new(-150, -150, -100, -100),
            enabled: false,
        });

        world.insert(Transform {
            value: TransformValue::scale(0.7, 0.7),
            source: expandable.untyped(),
            target: palette.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (-3.0, 3.0),
                Rectangle::new_half(Position::ZERO, Size::new(25, 25)),
                Position::ZERO,
            ),
            source: translatable.untyped(),
            target: expandable.untyped(),
        });

        world.queue_trigger(
            translatable,
            WidgetRectangle(Rectangle::new(-150, -150, -100, -100)),
        );

        world.observer(palette, move |&WidgetHsla(color), world| {
            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.front_color = color.into_color();
        });

        world.observer(expandable, move |&WidgetExpanded(expanded), world| {
            if let Ok(mut f) = world.enter_single_fetch_mut::<LayoutEnableAction>(palette) {
                (f.0)(world, expanded)
            }

            if let Ok(mut f) = world.enter_single_fetch_mut::<LayoutEnableAction>(translatable) {
                (f.0)(world, expanded)
            }
        });
    }
}

impl Element for ColorPicker {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        ColorPicker::insert(world);
    }
}
