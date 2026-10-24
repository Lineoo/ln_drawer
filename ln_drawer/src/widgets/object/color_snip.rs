use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, World};

use crate::{
    color::srgb::{mul_alpha, srgb_gamma_decode},
    layer::{
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        transform::{Transform, TransformValue},
        val::{Val, ValChanged},
    },
    measures::{FI64Ext, Rectangle},
    theme::Theme,
    tools::{
        mouse::MouseMenu,
        pointer::{PointerHover, PointerScroll},
    },
    widgets::{
        button::{ButtonClick, ButtonDrag, ButtonDragStatus, ToggleButton},
        common::WidgetCommon,
        renderer::rrect::RRect,
    },
};

pub struct ColorSnip {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub attr: ColorSnipAttr,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColorSnipAttr {
    pub position: IVec2,
    pub color: Vec4,
}

impl Element for ColorSnip {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("color_snip");
        world.set_autodeps(Some(this));

        let theme = world.single_fetch::<Theme>();
        let input = world.single::<LayerInput>();

        let button = world.insert(ToggleButton {
            common: self.common,
            rect: self.rect,
            theme: theme.button(),
            image: None,
            selected: false,
            hovering: false,
        });

        let color_rect = world.build(Transform::new(TransformValue::shrink(8, 8), self.rect));
        world.insert(RRect {
            common: self.common,
            rect: color_rect,
            order: 100,
            color: world.insert(Val::new(mul_alpha(srgb_gamma_decode(self.attr.color)))),
            radius: 16.0,
            width: 0.0,
        });

        world.observer(button, move |event: &PointerScroll, world| {
            world.queue_trigger(input, *event);
        });
        world.observer(button, move |event: &PointerHover, world| {
            world.queue_trigger(input, *event);
        });

        let mut from = None;
        world.observer(button, move |drag: &ButtonDrag, world| {
            let this = world.fetch(this);
            let mut rect = world.fetch_mut(this.rect);
            let source = from.get_or_insert(this.attr.position);
            let delta = drag.here.position - drag.from.position;
            let destination = *source + delta.q32_round();

            rect.val = Rectangle::new_half(destination, UVec2::splat(24));
            world.queue_trigger(rect.handle(), ValChanged);

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
    }
}
