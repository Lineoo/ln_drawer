pub mod brush_panel;
pub mod color_picker;
pub mod debug_panel;
pub mod desktop_ui;
pub mod layer_selection;
pub mod mobile_ui;
pub mod settings;
pub mod side_docker;

use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        transform::{Transform, TransformValue},
        val::Val,
    }, measures::Rectangle, theme::{Theme, ThemeVal}, tools::collider::ToolRectCollider, widgets::{common::WidgetCommon, renderer::rrect::RRect},
};

pub struct PanelAttr {
    pub round: bool,
    pub shadow: bool,
}

pub struct Panel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub attr: PanelAttr,
}

impl Panel {
    fn init(&self, world: &World, handle: Handle<Self>) {
        world.scope("panel");
        world.set_autodeps(Some(handle));
        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);

        let theme = world.single_fetch::<Theme>();
        let theme_val = world.single_fetch::<ThemeVal>();
        let shadow_offset = theme.shadow_offset.as_ivec2();
        let shadow_blur = theme.shadow_blur;
        let roundness = match self.attr.round {
            true => theme.roundness,
            false => 0.0,
        };

        world.insert(RRect {
            common: self.common,
            rect: self.rect,
            order: 0,
            color: theme_val.gray1,
            radius: roundness,
            width: 0.0,
        });

        if self.attr.shadow {
            let transform = TransformValue::shift(shadow_offset.x, shadow_offset.y);
            let shadow_rect = world.build(Transform::new(transform, self.rect));
            world.insert(RRect {
                common: self.common,
                rect: shadow_rect,
                order: -1,
                color: theme_val.shadow_color,
                radius: roundness,
                width: shadow_blur,
            });
        }

        world.insert(ToolRectCollider {
            common: self.common,
            target: handle.untyped(),
            rect: self.rect,
            order: 0,
        });
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
