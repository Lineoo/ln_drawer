pub mod brush_panel;
pub mod color_picker;
pub mod debug_panel;
pub mod desktop_ui;
pub mod layer_selection;
pub mod settings;
pub mod side_docker;

use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    animation::{AnimationDescriptor, AnimationType, SimpleAnimationDescriptor},
    measures::Rectangle,
    theme::Theme,
    tools::collider::ToolRectCollider,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible, common::WidgetCommon,
        renderer::rrect::RRect,
    },
};

pub struct Panel {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub shadow: bool,
}

impl Panel {
    pub fn init(&mut self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();
        let roundness = theme.roundness;
        let anim_factor = theme.anim_factor;
        let shadow_offset = theme.shadow_offset.as_ivec2();
        let shadow_blur = theme.shadow_blur;

        let back = world.insert(RRect {
            rect: self.rect,
            order: 0,
            color: theme.primary_color,
            radius: roundness,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let back_shadow = match self.shadow {
            true => Some(world.insert(RRect {
                rect: self.rect + shadow_offset,
                order: -1,
                color: theme.shadow_color,
                radius: roundness,
                width: shadow_blur,
                enabled: self.common.visible,
                camera: self.common.camera,
            })),
            false => None,
        };

        let back_rect_anim = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(self.rect, anim_factor),
            widget: back,
            action: move |_, world, rect| {
                world.queue_trigger(back, SetWidgetRectangle(rect));
                if let Some(back_shadow) = back_shadow {
                    world.queue_trigger(back_shadow, SetWidgetRectangle(rect + shadow_offset));
                }
            },
        });

        let collider = world.insert(ToolRectCollider {
            common: WidgetCommon {
                visible: self.common.visible,
                camera: self.common.camera,
            },
            target: handle.untyped(),
            rect: self.rect,
            order: 0,
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle);
            let mut back_rect_anim = world.fetch_mut(back_rect_anim);
            this.rect = rect;
            back_rect_anim.dst = rect.into_storage();
            back_rect_anim.src = rect.into_storage();
            world.queue_trigger(collider, SetWidgetRectangle(rect));
            world.queue_trigger(handle, WidgetRectangle(rect));
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle);
            this.common.visible = enabled;
            world.queue_trigger(back, SetWidgetVisible(enabled));
            if let Some(back_shadow) = back_shadow {
                world.queue_trigger(back_shadow, SetWidgetVisible(enabled));
            }
            world.queue_trigger(collider, SetWidgetVisible(enabled));
            world.queue_trigger(handle, WidgetVisible(enabled));
        });

        world.dependency(collider, handle);
        world.dependency(back, handle);
        if let Some(back_shadow) = back_shadow {
            world.dependency(back_shadow, handle);
        }
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
