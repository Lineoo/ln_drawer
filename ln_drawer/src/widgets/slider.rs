use glam::{IVec2, UVec2, Vec2};
use ln_world::{Descriptor, Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{Animation, AnimationType, DirectAnimation, SetAnimationDst},
    measures::{Axis, FI64Ext, Rectangle},
    render::rounded::{RoundedRect, RoundedRectDescriptor},
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::SetWidgetRectangle,
};

pub struct SetSliderValue(pub f32);
pub struct SliderValue(pub f32);

pub struct Slider {
    pub value: f32,
    pub axis: Axis,
    pub rect: Rectangle,
}

impl Slider {
    pub fn build(self, world: &World) -> Handle<Self> {
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.build(RoundedRectDescriptor {
            rect: back_rect(self.rect, self.axis),
            color: theme.secondary_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: true,
            order: 0,
        });

        let position = into_position(self.rect, self.axis, self.value);

        let front = world.build(RoundedRectDescriptor {
            rect: front_rect(self.rect, self.axis, position),
            color: theme.theme_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: true,
            order: 1,
        });

        let knob = world.build(RoundedRectDescriptor {
            rect: knob_rect(self.rect, self.axis, position),
            color: theme.primary_color,
            shadow_color: theme.shadow_color,
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: theme.shadow_blur,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 20,
            visible: true,
            order: 2,
        });

        let knob_split = world.build(RoundedRectDescriptor {
            rect: knob_split_rect(self.rect, self.axis, position),
            color: theme.secondary_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: 2.0,
            value: 2.0,
            vertex_extend: 0,
            visible: true,
            order: 3,
        });

        let back_rect_anim = world.build(DirectAnimation {
            init: back_rect(self.rect, self.axis),
            factor: theme.anim_factor,
            widget: back,
            access: |back| &mut back.desc.rect,
        });

        let knob_color_anim = world.build(DirectAnimation {
            init: theme.primary_color,
            factor: theme.anim_factor,
            widget: knob,
            access: |knob| &mut knob.desc.color,
        });

        let knob_split_color_anim = world.build(DirectAnimation {
            init: theme.primary_color,
            factor: theme.anim_factor,
            widget: knob_split,
            access: |split| &mut split.desc.color,
        });

        let collider = world.insert(ToolCollider {
            rect: back_rect(self.rect, self.axis),
            order: 10,
            enabled: true,
        });

        let handle = world.insert(self);

        world.observer(handle, move |&SetSliderValue(value), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut front = world.fetch_mut(front).unwrap();
            let mut knob = world.fetch_mut(knob).unwrap();
            let mut knob_split = world.fetch_mut(knob_split).unwrap();
            this.set_value(value, &mut front, &mut knob, &mut knob_split);
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            let mut front = world.fetch_mut(front).unwrap();
            let mut knob = world.fetch_mut(knob).unwrap();
            let mut knob_split = world.fetch_mut(knob_split).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.set_rect(
                rect,
                &mut back_rect_anim,
                &mut front,
                &mut knob,
                &mut knob_split,
                &mut collider,
            );
        });

        world.observer(collider, move |event: &PointerHit, world| {
            let this = world.fetch(handle).unwrap();
            let theme = world.single_fetch::<Theme>().unwrap();

            let position = match this.axis.is_vertical() {
                true => event.position.q32_round().y,
                false => event.position.q32_round().x,
            };

            let value = from_position(this.rect, this.axis, position);
            world.queue_trigger(handle, SliderValue(value));

            if let PointerHitStatus::Press = event.status {
                let back_rect_expanded = back_rect_expanded(this.rect, this.axis);
                world.trigger(back_rect_anim, &SetAnimationDst(back_rect_expanded));
                world.trigger(knob_color_anim, &SetAnimationDst(theme.highlight_color));
                world.trigger(
                    knob_split_color_anim,
                    &SetAnimationDst(theme.significant_color),
                );
            } else if let PointerHitStatus::Release = event.status {
                world.trigger(
                    back_rect_anim,
                    &SetAnimationDst(back_rect(this.rect, this.axis)),
                );
                world.trigger(knob_color_anim, &SetAnimationDst(theme.primary_color));
                world.trigger(
                    knob_split_color_anim,
                    &SetAnimationDst(theme.secondary_color),
                );
            }
        });

        handle
    }

    fn set_value(
        &mut self,
        value: f32,
        front: &mut RoundedRect,
        knob: &mut RoundedRect,
        knob_split: &mut RoundedRect,
    ) {
        self.value = value;

        let position = into_position(self.rect, self.axis, self.value);
        front.desc.rect = front_rect(self.rect, self.axis, position);
        knob.desc.rect = knob_rect(self.rect, self.axis, position);
        knob_split.desc.rect = knob_split_rect(self.rect, self.axis, position);
    }

    fn set_rect(
        &mut self,
        rect: Rectangle,
        back_rect_anim: &mut Animation<Rectangle>,
        front: &mut RoundedRect,
        knob: &mut RoundedRect,
        knob_split: &mut RoundedRect,
        collider: &mut ToolCollider,
    ) {
        self.rect = rect;

        let back_rect = back_rect(rect, self.axis);
        back_rect_anim.dst = back_rect.into_storage();
        back_rect_anim.src = back_rect.into_storage();
        collider.rect = back_rect;

        let position = into_position(self.rect, self.axis, self.value);
        front.desc.rect = front_rect(self.rect, self.axis, position);
        knob.desc.rect = knob_rect(self.rect, self.axis, position);
        knob_split.desc.rect = knob_split_rect(self.rect, self.axis, position);
    }
}

fn back_rect(rect: Rectangle, axis: Axis) -> Rectangle {
    front_rect(rect, axis, rect.axis_end(axis))
}

fn back_rect_expanded(rect: Rectangle, axis: Axis) -> Rectangle {
    const BAR_EX: i32 = 5;
    back_rect(rect, axis).expand(BAR_EX)
}

fn front_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    const BAR_HW: i32 = 6;
    Rectangle::axis_new(
        rect.axis_start(axis),
        rect.axis_center(axis.rotate()) - BAR_HW * axis.sign(),
        position,
        rect.axis_center(axis.rotate()) + BAR_HW * axis.sign(),
        axis,
    )
}

fn knob_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    const KNOB_SIZE: UVec2 = UVec2::new(9, 20);
    Rectangle::new_half(
        axis.vertical_position(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_extend(KNOB_SIZE),
    )
}

fn knob_split_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    const KNOB_SPLIT_SIZE: UVec2 = UVec2::new(6, 2);
    Rectangle::new_half(
        axis.vertical_position(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_extend(KNOB_SPLIT_SIZE),
    )
}

fn into_position(rect: Rectangle, axis: Axis, value: f32) -> i32 {
    let (knob_max, knob_min) = knob_limit(rect, axis);
    (value * (knob_max - knob_min) as f32).round() as i32 + knob_min
}

fn from_position(rect: Rectangle, axis: Axis, position: i32) -> f32 {
    let (knob_max, knob_min) = knob_limit(rect, axis);
    ((position - knob_min) as f32 / (knob_max - knob_min) as f32).clamp(0.0, 1.0)
}

fn knob_limit(rect: Rectangle, axis: Axis) -> (i32, i32) {
    const KNOB_EDGE_OFF: i32 = 17;
    (
        rect.axis_end(axis) - KNOB_EDGE_OFF * axis.sign(),
        rect.axis_start(axis) + KNOB_EDGE_OFF * axis.sign(),
    )
}

impl Element for Slider {}
impl Descriptor for Slider {
    type Target = Handle<Self>;
    fn when_build(self, world: &World) -> Self::Target {
        self.build(world)
    }
}
