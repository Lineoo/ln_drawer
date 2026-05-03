use ln_world::{Element, Handle, World};
use palette::{Hsla, IntoColor, RgbHue};

use crate::{
    layout::{
        LayoutEnableAction,
        transform::{Transform, TransformValue},
    },
    measures::{Position, Rectangle, Size},
    stroke::{StrokeLayer, modifier::Modifier},
    widgets::{
        WidgetClick, WidgetEnabled, WidgetExpanded, WidgetHsla, WidgetRectangle, button::Button,
        expandable::Expandable, palette::hsl::PaletteHsl, translatable::Translatable,
    },
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

        let button = world.insert(Button {
            rect: Rectangle::new(-150, -150, -100, -100),
            order: 100,
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
                (0.0, 0.0),
                Rectangle::new_half(Position::ZERO, Size::new(25, 25)),
                Position::new(40, 40),
            ),
            source: expandable.untyped(),
            target: button.untyped(),
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
            layer.modifier.color = color.into_color();
        });

        let mut harder = false;
        world.observer(button, move |&WidgetClick, world| {
            harder = !harder;
            let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
            stroke.modifier = if harder {
                Modifier {
                    min_size: 0.0,
                    max_size: 6.0,
                    size_force_exp: 1.0,
                    min_flow: 0.7,
                    max_flow: 1.0,
                    flow_force_exp: 2.0,
                    softness: 0.2,
                    ..stroke.modifier
                }
            } else {
                Modifier {
                    min_size: 1.0,
                    max_size: 25.0,
                    size_force_exp: 1.0,
                    min_flow: 0.1,
                    max_flow: 1.0,
                    flow_force_exp: 1.0,
                    softness: 0.5,
                    ..stroke.modifier
                }
            };
        });
        world.queue_trigger(button, WidgetClick);

        world.observer(expandable, move |&WidgetExpanded(expanded), world| {
            if let Ok(mut f) = world.enter_single_fetch_mut::<LayoutEnableAction>(palette) {
                (f.0)(world, expanded)
            }

            world.queue_trigger(button, WidgetEnabled(expanded));

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
