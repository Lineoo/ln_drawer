use ln_world::{Element, Handle, World};
use palette::{Hsla, IntoColor, RgbHue};

use crate::{
    layout::transform::{Transform, TransformValue},
    measures::{Position, Rectangle, Size},
    stroke::{StrokeLayer, modifier::Modifier},
    widgets::{
        WidgetAnimatedRectangle, WidgetClick, WidgetEnabled, WidgetHsla, WidgetRectangle,
        button::{Button, ButtonDrag, ButtonDragStatus},
        palette::hsl::PaletteHsl,
    },
};

pub mod hsl;

pub struct ColorPicker {
    rect: Rectangle,
    transform: TransformValue,
    expanded: bool,
}

impl ColorPicker {
    fn insert(&mut self, world: &World, this: Handle<Self>) {
        let palette = world.insert(PaletteHsl {
            rect: Rectangle::new(100, 100, 300, 300),
            color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
            enabled: false,
        });

        let main_panel = world.insert(Button {
            rect: Rectangle::new(-100, -100, -50, -50),
            order: 0,
        });

        let brush_button = world.insert(Button {
            rect: Rectangle::new(-150, -150, -100, -100),
            order: 100,
        });

        let render_debug_button = world.insert(Button {
            rect: Rectangle::new(-150, -150, -100, -100),
            order: 100,
        });

        world.queue_trigger(brush_button, WidgetEnabled(false));
        world.queue_trigger(render_debug_button, WidgetEnabled(false));

        world.insert(Transform {
            value: TransformValue::scale(0.7, 0.7),
            source: main_panel.untyped(),
            target: palette.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.0, 0.0),
                Rectangle::new_half(Position::ZERO, Size::new(25, 25)),
                Position::new(40, 40),
            ),
            source: main_panel.untyped(),
            target: brush_button.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.0, 0.0),
                Rectangle::new_half(Position::ZERO, Size::new(25, 25)),
                Position::new(40, 100),
            ),
            source: main_panel.untyped(),
            target: render_debug_button.untyped(),
        });

        world.observer(palette, move |&WidgetHsla(color), world| {
            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.modifier.color = color.into_color();
        });

        let mut kind = 2;
        world.observer(brush_button, move |&WidgetClick, world| {
            kind = (kind + 1) % 3;
            let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
            stroke.modifier = match kind {
                0 => Modifier {
                    min_size: 0.0,
                    max_size: 6.0,
                    size_force_exp: 1.0,
                    min_flow: 0.7,
                    max_flow: 1.0,
                    flow_force_exp: 2.0,
                    softness: 0.2,
                    ..stroke.modifier
                },
                1 => Modifier {
                    min_size: 1.0,
                    max_size: 25.0,
                    size_force_exp: 1.0,
                    min_flow: 0.1,
                    max_flow: 1.0,
                    flow_force_exp: 1.0,
                    softness: 0.5,
                    ..stroke.modifier
                },
                2 => Modifier {
                    min_size: 0.5,
                    max_size: 0.5,
                    size_force_exp: 0.0,
                    min_flow: 1.0,
                    max_flow: 1.0,
                    flow_force_exp: 0.0,
                    softness: 0.0,
                    ..stroke.modifier
                },
                _ => unreachable!(),
            };
            stroke.shape = match kind {
                0 | 1 => 0,
                2 => 1,
                _ => unreachable!(),
            };
        });
        world.queue_trigger(brush_button, WidgetClick);

        world.observer(render_debug_button, |&WidgetClick, world| {
            let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
            stroke.render_debugging = !stroke.render_debugging;
        });

        let mut drag_start = None;
        world.observer(main_panel, move |drag: &ButtonDrag, world| {
            let mut this = world.fetch_mut(this).unwrap();
            if drag.status == ButtonDragStatus::Start {
                drag_start = Some(this.rect);
            }

            if let Some(start) = drag_start {
                let rect = start + (drag.here.position - drag.from.position).round();
                this.rect = rect;
                let expanded = match this.expanded {
                    false => rect,
                    true => this.transform.compute(rect),
                };
                world.queue_trigger(main_panel, WidgetRectangle(expanded));
            }
        });

        world.observer(main_panel, move |&WidgetClick, world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.expanded = !this.expanded;

            if this.expanded {
                world.queue_trigger(
                    main_panel,
                    WidgetAnimatedRectangle(this.transform.compute(this.rect)),
                );
            } else {
                world.queue_trigger(main_panel, WidgetAnimatedRectangle(this.rect));
            }

            world.queue_trigger(palette, WidgetEnabled(this.expanded));
            world.queue_trigger(brush_button, WidgetEnabled(this.expanded));
            world.queue_trigger(render_debug_button, WidgetEnabled(this.expanded));
        });
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(-100, -100, -50, -50),
            transform: TransformValue::scale(10.0, 10.0),
            expanded: false,
        }
    }
}

impl Element for ColorPicker {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.insert(world, this);
    }
}
