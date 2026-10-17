use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    color::srgb::{mul_alpha, srgb_gamma_decode},
    layer::{
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        echo::Echo,
        transfer::Transfer,
        transform::{Transform, TransformValue},
    },
    measures::{FI64Ext, Rectangle},
    theme::Theme,
    tools::mouse::MouseMenu,
    widgets::{
        SetWidgetRectangle,
        button::{ButtonClick, ButtonDrag, ButtonDragStatus, ToggleButton, ToggleButtonTheme},
        common::WidgetCommon,
        renderer::rrect::RRect,
    },
};

pub struct ColorSnip {
    pub common: WidgetCommon,
    pub attr: ColorSnipAttr,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColorSnipAttr {
    pub position: IVec2,
    pub color: Vec4,
}

impl Element for ColorSnip {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let input = world.single::<LayerInput>();

        let button = world.insert(ToggleButton {
            common: self.common,
            rect: Rectangle::new_half(self.attr.position, UVec2::splat(24)),
            theme: ToggleButtonTheme {
                idle_color: theme.primary_color,
                hover_color: theme.secondary_color,
                press_color: theme.highlight_color,
                selected_color: theme.highlight_color,
            },
            image: None,
            selected: false,
            hovering: false,
        });

        let color = world.insert(RRect {
            rect: Rectangle::new_half(self.attr.position, UVec2::splat(16)),
            order: 100,
            color: mul_alpha(srgb_gamma_decode(self.attr.color)),
            radius: 16.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        world.insert(Transform {
            value: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(24)),
            ),
            source: this.untyped(),
            target: button.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::shrink(8, 8),
            source: button.untyped(),
            target: color.untyped(),
        });

        Echo::new(world, this).widget_rectangle().widget_visible();
        Echo::new(world, button).widget_rectangle().widget_visible();
        Transfer::new(world, button, input)
            .pointer_scroll()
            .pointer_hover();

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.attr.position = rect.center();
        });

        let mut from = None;
        world.observer(button, move |drag: &ButtonDrag, world| {
            let this = world.fetch(this);
            let source = from.get_or_insert(this.attr.position);
            let delta = drag.here.position - drag.from.position;
            let destination = *source + delta.q32_round();

            world.queue_trigger(
                this.handle(),
                SetWidgetRectangle(Rectangle::new_half(destination, UVec2::splat(24))),
            );

            if drag.status == ButtonDragStatus::End {
                from = None;
            }
        });

        world.observer(button, move |ButtonClick, world| {
            let mut page = world.single_fetch_mut::<LayerPage>();
            let this = world.fetch(this);
            page.set_color(this.attr.color);
            world.queue_trigger(page.handle(), BrushConfigurationChanged);
        });

        world.observer(button, move |&MouseMenu(_), world| {
            world.queue(move |world| {
                world.remove(this);
            });
        });

        world.dependency(button, this);
        world.dependency(color, this);
    }
}
