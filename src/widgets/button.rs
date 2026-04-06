use palette::Srgba;

use crate::{
    animation::{AnimationDescriptor, AnimationValue},
    layout::{
        LayoutRectangleAction,
        transform::{Transform, TransformValue},
    },
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Luni,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{WidgetButton, WidgetClick, WidgetHover, WidgetRectangle},
    world::{Element, Handle, World},
};

pub struct Button {
    pub rect: Rectangle,
    pub order: isize,
}

impl Button {
    fn attach_luni(&mut self, world: &World, this: Handle<Self>) {
        let luni = world.single_fetch::<Luni>().unwrap();

        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            order: self.order,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        let frame_anim_color = world.build(AnimationDescriptor {
            src: Srgba::new(0.0, 0.0, 0.0, 0.0),
            dst: luni.color,
            factor: luni.anim_factor,
        });

        world.observer(frame_anim_color, move |&AnimationValue(value), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.color = value;
        });

        // dependency

        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        world.observer(this, move |event: &WidgetHover, world| {
            let luni = world.single_fetch::<Luni>().unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(this, move |event: &WidgetButton, world| {
            let luni = world.single_fetch::<Luni>().unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = luni.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = luni.active_color,
            }
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: self.order,
            enabled: true,
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: this.untyped(),
            target: collider.untyped(),
        });

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

        world.dependency(collider, this);
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        world.enter_insert(
            this,
            LayoutRectangleAction(Box::new(move |world, rect| {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = rect;
                rect
            })),
        );
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            order: 10,
        }
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_layout(world, this);
        self.attach_luni(world, this);
        self.attach_pointer(world, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.trigger(this, &WidgetRectangle(self.rect));
    }
}
