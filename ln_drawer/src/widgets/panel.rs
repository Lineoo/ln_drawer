use ln_world::{Element, Handle, World};

use crate::{
    animation::{AnimationType, DirectAnimation},
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    tools::collider::ToolCollider,
    widgets::{WidgetEnabled, WidgetRectangle},
};

pub struct Panel {
    pub rect: Rectangle,
    pub visible: bool,
}

pub struct PanelAnimation {
    pub src: Rectangle,
    pub dst: Rectangle,
    pub hidden_after_finished: bool,
}

impl Panel {
    pub fn receive_event(panel: Handle<Panel>, world: &World) {
        world.observer(panel, move |&WidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(panel).unwrap();
            this.rect = rect;
        });

        world.observer(panel, move |&WidgetEnabled(enabled), world| {
            let mut this = world.fetch_mut(panel).unwrap();
            this.visible = enabled;
        });
    }

    pub fn create_renderer(panel: Handle<Panel>, world: &World) {
        let this = world.fetch(panel).unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.build(RoundedRectDescriptor {
            rect: this.rect,
            color: theme.primary_color,
            shadow_color: theme.shadow_color,
            shadow_offset: theme.shadow_offset,
            shadow_blur: theme.shadow_blur,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: theme.shadow_blur.ceil() as i32
                + theme.shadow_offset.abs().max_element().ceil() as i32,
            visible: this.visible,
            order: 0,
        });

        let back_rect_anim = world.build(DirectAnimation {
            init: this.rect,
            factor: theme.anim_factor,
            widget: back,
            access: |back| &mut back.desc.rect,
        });

        world.observer(panel, move |&WidgetRectangle(rect), world| {
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            back_rect_anim.dst = rect.into_storage();
            back_rect_anim.src = rect.into_storage();
        });

        world.observer(panel, move |&WidgetEnabled(enabled), world| {
            let mut back = world.fetch_mut(back).unwrap();
            back.desc.visible = enabled;
        });

        world.observer(panel, move |anim: &PanelAnimation, world| {
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            back_rect_anim.dst = anim.dst.into_storage();
            back_rect_anim.src = anim.src.into_storage();
        });
    }

    pub fn create_interact(panel: Handle<Panel>, world: &World) {
        let this = world.fetch(panel).unwrap();

        let collider = world.insert(ToolCollider {
            rect: this.rect,
            order: 10,
            enabled: this.visible,
        });

        world.observer(panel, move |&WidgetRectangle(rect), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.rect = rect;
        });

        world.observer(panel, move |&WidgetEnabled(enabled), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = enabled;
        });
    }
}

impl Element for Panel {}
