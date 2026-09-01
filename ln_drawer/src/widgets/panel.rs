pub mod color_picker;
pub mod debug_panel;
pub mod settings;
pub mod side_docker;

use ln_world::{Element, Handle, World};

use crate::{
    animation::{AnimationDescriptor, AnimationType, SimpleAnimationDescriptor},
    measures::Rectangle,
    theme::Theme,
    tools::collider::ToolCollider,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
        renderer::rrect::RRect,
    },
};

pub struct Panel {
    pub rect: Rectangle,
    pub visible: bool,
    pub shadow: bool,
}

pub struct SetPanelAnimation {
    pub src: Rectangle,
    pub dst: Rectangle,
    #[expect(unused)]
    pub hidden_after_finished: bool,
}

impl Panel {
    pub fn init(&mut self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();
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
            enabled: self.visible,
        });

        let back_shadow = match self.shadow {
            true => Some(world.insert(RRect {
                rect: self.rect + shadow_offset,
                order: -1,
                color: theme.shadow_color,
                radius: roundness,
                width: shadow_blur,
                enabled: self.visible,
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

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 0,
            enabled: self.visible,
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            back_rect_anim.dst = rect.into_storage();
            back_rect_anim.src = rect.into_storage();
            collider.rect = rect;
            world.queue_trigger(handle, WidgetRectangle(rect));
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = enabled;
            world.queue_trigger(back, SetWidgetVisible(enabled));
            if let Some(back_shadow) = back_shadow {
                world.queue_trigger(back_shadow, SetWidgetVisible(enabled));
            }
            collider.enabled = enabled;
            world.queue_trigger(handle, WidgetVisible(enabled));
        });

        world.observer(handle, move |anim: &SetPanelAnimation, world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            this.rect = anim.dst;
            back_rect_anim.dst = anim.dst.into_storage();
            back_rect_anim.src = anim.src.into_storage();
            world.queue_trigger(handle, WidgetRectangle(anim.dst));
        });
    }
}

impl Element for Panel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
