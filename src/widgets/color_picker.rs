use palette::Hsla;

use crate::{
    animation::{AnimationDescriptor, SimpleAnimationDescriptor},
    layout::{LayoutRectangle, transform::Transform},
    measures::{Rectangle, Size},
    render::rounded::RoundedRectDescriptor,
    theme::Luni,
    tools::pointer::{
        PointerCollider, PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus,
    },
    widgets::{WidgetButton, WidgetClick, WidgetDestroyed, WidgetHover, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct ColorPicker {
    pub rect: Rectangle,
    pub color: Hsla,
}

impl Element for ColorPicker {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.receive_layout(world, this);
        self.attach_luni(world, this);
        self.attach_pointer(world, this);
        self.attach_default_behavior(world, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetRectangle(self.rect));
    }
}

impl ColorPicker {
    fn receive_layout(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&LayoutRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });
    }

    fn attach_luni(&mut self, world: &World, this: Handle<Self>) {
        let luni = world.single_fetch::<Luni>().unwrap();

        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            visible: true,
            order: 10,
        });

        let frame_rect = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(
                [self.rect.width() as f32, self.rect.height() as f32],
                luni.anim_factor,
            ),
            widget: frame,
            action: |mut frame, _, extend| {
                frame.rect.extend = Size::new(extend[0].floor() as u32, extend[1].floor() as u32);
            },
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            frame_rect.dst = [rect.width() as f32, rect.height() as f32];
        });

        world.observer(this, move |&WidgetDestroyed, world| {
            world.remove(frame).unwrap();
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: self.rect,
            order: 10,
            enabled: true,
        });

        world.insert(Transform::copy(this.untyped(), collider.untyped()));
        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world| {
            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                }
                PointerHitStatus::Release => {
                    world.trigger(this, &WidgetClick);
                    world.trigger(this, &WidgetButton::ButtonRelease);
                }
                _ => {}
            }
        });

        world.observer(collider, move |event: &PointerHover, world| {
            match event.status {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::HoverLeave);
                }
                _ => {}
            }
        });
    }

    fn attach_default_behavior(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&WidgetClick, world| {
            let mut this = world.fetch_mut(this).unwrap();
            if this.is_expanded() {
                this.rect.extend = Size::new(30, 30);
            } else {
                this.rect.extend = Size::new(400, 800);
            }
        });
    }

    fn is_expanded(&self) -> bool {
        self.rect.width() >= 400 && self.rect.height() >= 800
    }
}
