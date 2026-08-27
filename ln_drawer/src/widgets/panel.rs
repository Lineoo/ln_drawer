pub mod color_picker;
pub mod debug_panel;
pub mod settings;
pub mod side_docker;

use glam::Vec2;
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{AnimationType, DirectAnimation},
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    tools::collider::ToolCollider,
    widgets::{SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible},
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

        let back = match self.shadow {
            true => world.build(RoundedRectDescriptor {
                rect: self.rect,
                color: theme.primary_color,
                shadow_color: theme.shadow_color,
                shadow_offset: theme.shadow_offset,
                shadow_blur: theme.shadow_blur,
                shrink: theme.roundness,
                value: theme.roundness,
                vertex_extend: theme.shadow_blur.ceil() as i32
                    + theme.shadow_offset.abs().max_element().ceil() as i32,
                visible: self.visible,
                order: 0,
            }),
            false => world.build(RoundedRectDescriptor {
                rect: self.rect,
                color: theme.primary_color,
                shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
                shadow_offset: Vec2::ZERO,
                shadow_blur: 0.0,
                shrink: theme.roundness,
                value: theme.roundness,
                vertex_extend: 0,
                visible: self.visible,
                order: 0,
            }),
        };

        let back_rect_anim = world.build(DirectAnimation {
            init: self.rect,
            factor: theme.anim_factor,
            widget: back,
            access: |back| &mut back.desc.rect,
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
            let mut back = world.fetch_mut(back).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = enabled;
            back.desc.visible = enabled;
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
