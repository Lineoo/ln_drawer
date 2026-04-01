use palette::{Hsla, IntoColor, RgbHue};

use crate::{
    layout::{
        LayoutControls,
        transform::{Transform, TransformValue},
    },
    measures::{Position, Rectangle, Size},
    stroke::StrokeLayer,
    widgets::{
        WidgetExpanded, WidgetHsla, expandable::Expandable, palette::hsl::PaletteHsl,
        translatable::Translatable,
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
        });

        let expandable = world.insert(Expandable {
            rect: Rectangle::new(-100, -100, -50, -50),
            transform: TransformValue::scale(10.0, 10.0),
            expanded: false,
        });

        let translatable = world.insert(Translatable {
            rect: Rectangle::new(-150, -150, -100, -100),
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

        world.observer(palette, move |&WidgetHsla(color), world| {
            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.front_color = color.into_color();
        });

        world.observer(expandable, move |&WidgetExpanded(expanded), world| {
            let controls = world.single_fetch::<LayoutControls>().unwrap();
            if let Some(&control) = controls.0.get(&palette.untyped())
                && let Some(enable) = &mut world.fetch_mut(control).unwrap().enabled
            {
                enable(world, expanded);
            }

            let controls = world.single_fetch::<LayoutControls>().unwrap();
            if let Some(&control) = controls.0.get(&translatable.untyped())
                && let Some(enable) = &mut world.fetch_mut(control).unwrap().enabled
            {
                enable(world, expanded);
            }
        });
    }
}

impl Element for ColorPicker {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        ColorPicker::insert(world);
    }
}
