use ln_world::{Element, Handle, World};

use crate::{
    layout::transform::TransformValue,
    measures::{FI64Ext, Rectangle},
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerScroll},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
        renderer::rrect::RRect,
    },
};

pub struct Container {
    pub rect: Rectangle,
    pub inner: Rectangle,
    pub inner_transform: TransformValue,
    pub visible: bool,
}

impl Container {
    pub fn init(&mut self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.insert(RRect {
            rect: self.rect,
            order: 0,
            color: theme.primary_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: self.visible,
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 0,
            enabled: self.visible,
        });

        let mut status = None;
        world.observer(collider, move |event: &PointerHit, world| {
            status = match (status, event.status) {
                (None, PointerHitStatus::Press | PointerHitStatus::Moving) => Some(event.position),
                (None, PointerHitStatus::Release) => None,
                (Some(_), PointerHitStatus::Press) => Some(event.position),
                (Some(p), PointerHitStatus::Moving) => {
                    let mut this = world.fetch_mut(handle).unwrap();
                    this.inner += (event.position - p).q32_round();
                    this.inner = this.inner.adjust_contain(this.rect);
                    world.queue_trigger(handle, WidgetRectangle(this.inner));
                    Some(event.position)
                }
                (Some(p), PointerHitStatus::Release) => {
                    let mut this = world.fetch_mut(handle).unwrap();
                    this.inner += (event.position - p).q32_round();
                    this.inner = this.inner.adjust_contain(this.rect);
                    world.queue_trigger(handle, WidgetRectangle(this.inner));
                    None
                }
            }
        });

        world.observer(collider, move |event: &PointerScroll, world| {
            let mut this = world.fetch_mut(handle).unwrap();
            this.inner += event.delta.round().as_ivec2();
            this.inner = this.inner.adjust_contain(this.rect);
            world.queue_trigger(handle, WidgetRectangle(this.inner));
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let this = &mut *world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.inner += rect.origin - this.rect.origin;
            this.inner.extend = this.inner_transform.compute(rect).extend;
            this.inner = this.inner.adjust_contain(rect);
            this.rect = rect;
            collider.rect = rect;
            world.queue_trigger(back, SetWidgetRectangle(rect));
            world.queue_trigger(handle, WidgetRectangle(this.inner));
        });

        world.observer(handle, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = enabled;
            collider.enabled = enabled;
            world.queue_trigger(back, SetWidgetVisible(enabled));
            world.queue_trigger(handle, WidgetVisible(enabled));
        });
    }
}

impl Element for Container {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
