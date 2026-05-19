use ln_world::{Element, Handle, World};
use palette::{Hsla, IntoColor, RgbHue};

use crate::{
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    stroke::StrokeLayer,
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
            ..Default::default()
        });

        world.insert(Transform {
            value: TransformValue::scale(0.7, 0.7),
            source: main_panel.untyped(),
            target: palette.untyped(),
        });

        world.observer(palette, move |&WidgetHsla(color), world| {
            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.modifier.color = color.into_color();
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
