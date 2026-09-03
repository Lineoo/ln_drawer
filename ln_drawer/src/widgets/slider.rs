use cosmic_text::{Align, Metrics};
use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, World};

use crate::{
    animation::{
        Animation, AnimationDescriptor, AnimationType, SetAnimationDst, SimpleAnimationDescriptor,
    },
    measures::{Axis, FI64Ext, Rectangle},
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetHover,
        renderer::{
            rrect::{RRect, SetRRectColor},
            text::{SetText, Text},
        },
    },
};

pub struct SliderValue(pub f32);
pub struct SetSliderValue(pub f32);

pub struct Slider {
    pub value: f32,
    pub axis: Axis,
    pub rect: Rectangle,
    pub pressed: bool,
}

pub struct SliderLabel {
    pub text: String,
    pub clockwise: bool,
    pub source: Handle<Slider>,
    pub hover: bool,
    pub visible: bool,
}

impl Slider {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.insert(RRect {
            rect: back_rect(self.rect, self.axis),
            order: 20,
            color: theme.secondary_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: true,
        });

        let position = into_position(self.rect, self.axis, self.value);

        let front = world.insert(RRect {
            rect: front_rect(self.rect, self.axis, position),
            order: 21,
            color: theme.theme_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: true,
        });

        let knob = world.insert(RRect {
            rect: knob_rect(self.rect, self.axis, position),
            order: 22,
            color: theme.blank_color,
            radius: theme.roundness,
            width: 0.0,
            enabled: true,
        });

        let knob_split = world.insert(RRect {
            rect: knob_split_rect(self.rect, self.axis, position),
            order: 23,
            color: theme.primary_color,
            radius: 2.0,
            width: 0.0,
            enabled: true,
        });

        let back_rect_anim = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(back_rect(self.rect, self.axis), theme.anim_factor),
            widget: back,
            action: move |_, world, rect| {
                world.queue_trigger(back, SetWidgetRectangle(rect));
            },
        });

        let knob_color_anim = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(theme.primary_color, theme.anim_factor),
            widget: knob,
            action: move |_, world, color| {
                world.queue_trigger(knob, SetRRectColor(color));
            },
        });

        let knob_split_color_anim = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(theme.primary_color, theme.anim_factor),
            widget: knob_split,
            action: move |_, world, color| {
                world.queue_trigger(knob_split, SetRRectColor(color));
            },
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 10,
            enabled: true,
        });

        world.observer(handle, move |&SetSliderValue(value), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            this.set_value(world, value, front, knob, knob_split);
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.set_rect(
                world,
                rect,
                &mut back_rect_anim,
                front,
                knob,
                knob_split,
                &mut collider,
            );
        });

        world.observer(handle, move |&SetWidgetVisible(visible), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            world.queue_trigger(back, SetWidgetVisible(visible));
            world.queue_trigger(front, SetWidgetVisible(visible));
            world.queue_trigger(knob, SetWidgetVisible(visible));
            world.queue_trigger(knob_split, SetWidgetVisible(visible));
            collider.enabled = visible;
        });

        world.observer(collider, move |event: &PointerHover, world| {
            if let PointerHoverStatus::Enter = event.status {
                world.queue_trigger(handle, WidgetHover::Enter);
            } else if let PointerHoverStatus::Leave = event.status {
                world.queue_trigger(handle, WidgetHover::Leave);
            }
        });

        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let theme = world.single_fetch::<Theme>().unwrap();

            let position = match this.axis.is_vertical() {
                true => event.position.q32_round().y,
                false => event.position.q32_round().x,
            };

            let value = from_position_pressed(this.rect, this.axis, position);
            world.queue_trigger(handle, SliderValue(value));

            if let PointerHitStatus::Press = event.status {
                this.pressed = true;
                let back_rect_expanded = back_rect_pressed(this.rect, this.axis);
                world.trigger(back_rect_anim, &SetAnimationDst(back_rect_expanded));
                world.trigger(knob_color_anim, &SetAnimationDst(theme.primary_color));
                world.trigger(knob_split_color_anim, &SetAnimationDst(theme.theme_color));
            } else if let PointerHitStatus::Release = event.status {
                this.pressed = false;
                let back_rect = back_rect(this.rect, this.axis);
                world.trigger(back_rect_anim, &SetAnimationDst(back_rect));
                world.trigger(knob_color_anim, &SetAnimationDst(theme.blank_color));
                world.trigger(knob_split_color_anim, &SetAnimationDst(theme.primary_color));
            }
        });
    }

    fn set_value(
        &mut self,
        world: &World,
        value: f32,
        front: Handle<RRect>,
        knob: Handle<RRect>,
        knob_split: Handle<RRect>,
    ) {
        self.value = value;

        let position = match self.pressed {
            true => into_position_pressed(self.rect, self.axis, self.value),
            false => into_position(self.rect, self.axis, self.value),
        };

        world.queue_trigger(
            front,
            SetWidgetRectangle(front_rect(self.rect, self.axis, position)),
        );
        world.queue_trigger(
            knob_split,
            SetWidgetRectangle(knob_split_rect(self.rect, self.axis, position)),
        );

        match self.pressed {
            true => world.queue_trigger(
                knob,
                SetWidgetRectangle(knob_rect_pressed(self.rect, self.axis, position)),
            ),
            false => world.queue_trigger(
                knob,
                SetWidgetRectangle(knob_rect(self.rect, self.axis, position)),
            ),
        }
    }

    fn set_rect(
        &mut self,
        world: &World,
        rect: Rectangle,
        back_rect_anim: &mut Animation<Rectangle>,
        front: Handle<RRect>,
        knob: Handle<RRect>,
        knob_split: Handle<RRect>,
        collider: &mut ToolCollider,
    ) {
        self.rect = rect;
        collider.rect = rect;

        let back_rect = back_rect(rect, self.axis);
        back_rect_anim.dst = back_rect.into_storage();
        back_rect_anim.src = back_rect.into_storage();

        let position = into_position(self.rect, self.axis, self.value);
        world.queue_trigger(
            front,
            SetWidgetRectangle(front_rect(self.rect, self.axis, position)),
        );
        world.queue_trigger(
            knob_split,
            SetWidgetRectangle(knob_split_rect(self.rect, self.axis, position)),
        );

        match self.pressed {
            true => world.queue_trigger(
                knob,
                SetWidgetRectangle(knob_rect_pressed(self.rect, self.axis, position)),
            ),
            false => world.queue_trigger(
                knob,
                SetWidgetRectangle(knob_rect(self.rect, self.axis, position)),
            ),
        }
    }
}

impl SliderLabel {
    fn init(&self, world: &World, this: Handle<Self>) {
        let slider = world.fetch(self.source).unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let position = into_position(slider.rect, slider.axis, slider.value);

        let back = world.insert(RRect {
            rect: label_back_rect(slider.rect, slider.axis, self.clockwise, position),
            order: 100,
            color: theme.blank_color,
            radius: LABEL_HALF.y as f32,
            width: 0.0,
            enabled: self.visible && self.hover,
        });

        let label = world.insert(Text {
            text: self.text.clone(),
            order: 150,
            metrics: Metrics::new(12., 2. * LABEL_HALF.y as f32),
            align: Align::Center,
            visible: self.visible && self.hover,
            ..Default::default()
        });

        world.observer(self.source, move |event: &WidgetHover, world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.hover = match event {
                WidgetHover::Enter => true,
                WidgetHover::Leave => false,
            };
            world.queue_trigger(back, SetWidgetVisible(this.hover && this.visible));
            world.queue_trigger(label, SetWidgetVisible(this.hover && this.visible));
        });

        world.observer(self.source, move |&SetWidgetRectangle(rect), world| {
            let this = world.fetch(this).unwrap();
            let slider = world.fetch(this.source).unwrap();
            let position = into_position_pressed(rect, slider.axis, slider.value);
            let rect = label_back_rect(rect, slider.axis, this.clockwise, position);
            world.queue_trigger(back, SetWidgetRectangle(rect));
            world.queue_trigger(label, SetWidgetRectangle(rect));
        });

        world.observer(self.source, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.visible = visible;
            world.queue_trigger(back, SetWidgetVisible(this.hover && this.visible));
            world.queue_trigger(label, SetWidgetVisible(this.hover && this.visible));
        });

        world.observer(self.source, move |&SetSliderValue(val), world| {
            let this = world.fetch(this).unwrap();
            let slider = world.fetch(this.source).unwrap();
            let position = into_position_pressed(slider.rect, slider.axis, val);
            let rect = label_back_rect(slider.rect, slider.axis, this.clockwise, position);
            world.queue_trigger(back, SetWidgetRectangle(rect));
            world.queue_trigger(label, SetWidgetRectangle(rect));
        });

        world.observer(this, move |val: &SetText, world| {
            world.trigger(label, val);
        });

        world.queue_trigger(this, SetSliderValue(slider.value));
    }
}

const BAR_EX: i32 = 5;
const BAR_HW: i32 = 6;
const KNOB_SIZE: UVec2 = UVec2::new(8, 16);
const KNOB_SIZE_PRESSED: UVec2 = UVec2::new(7, 16);
const KNOB_SPLIT_SIZE: UVec2 = UVec2::new(5, 2);
const LABEL_GAP: i32 = 2;
const LABEL_HALF: UVec2 = UVec2::new(30, 12);

fn back_rect(rect: Rectangle, axis: Axis) -> Rectangle {
    front_rect(rect, axis, rect.axis_end(axis))
}

fn back_rect_pressed(rect: Rectangle, axis: Axis) -> Rectangle {
    back_rect(rect, axis).expand(BAR_EX)
}

fn front_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    Rectangle::axis_new(
        rect.axis_start(axis),
        rect.axis_center(axis.rotate()) - BAR_HW * axis.sign(),
        position,
        rect.axis_center(axis.rotate()) + BAR_HW * axis.sign(),
        axis,
    )
}

fn knob_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    Rectangle::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SIZE),
    )
}

fn knob_rect_pressed(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    Rectangle::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SIZE_PRESSED),
    )
}

fn knob_split_rect(rect: Rectangle, axis: Axis, position: i32) -> Rectangle {
    Rectangle::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SPLIT_SIZE),
    )
}

fn into_position(rect: Rectangle, axis: Axis, value: f32) -> i32 {
    let (knob_max, knob_min) = knob_limit(rect, axis);
    (value * (knob_max - knob_min) as f32).round() as i32 + knob_min
}

fn into_position_pressed(rect: Rectangle, axis: Axis, value: f32) -> i32 {
    let (knob_max, knob_min) = knob_limit_pressed(rect, axis);
    (value * (knob_max - knob_min) as f32).round() as i32 + knob_min
}

fn from_position_pressed(rect: Rectangle, axis: Axis, position: i32) -> f32 {
    let (knob_max, knob_min) = knob_limit_pressed(rect, axis);
    ((position - knob_min) as f32 / (knob_max - knob_min) as f32).clamp(0.0, 1.0)
}

fn knob_limit(rect: Rectangle, axis: Axis) -> (i32, i32) {
    (
        rect.axis_end(axis) - KNOB_SIZE.y as i32 * axis.sign(),
        rect.axis_start(axis) + KNOB_SIZE.y as i32 * axis.sign(),
    )
}

fn knob_limit_pressed(rect: Rectangle, axis: Axis) -> (i32, i32) {
    (
        rect.axis_end(axis) - KNOB_SIZE_PRESSED.y as i32 * axis.sign(),
        rect.axis_start(axis) + KNOB_SIZE_PRESSED.y as i32 * axis.sign(),
    )
}

fn label_back_rect(rect: Rectangle, axis: Axis, clockwise: bool, position: i32) -> Rectangle {
    let raxis = match clockwise {
        true => axis.rotate(),
        false => axis.rotate_rev(),
    };
    let offset = BAR_HW + BAR_EX + LABEL_GAP + axis.horizontal_uvec2(LABEL_HALF).y as i32;
    Rectangle::new_half(
        axis.horizontal_ivec2(IVec2::new(position, rect.axis_center(axis.rotate())))
            + raxis.vertical_ivec2(IVec2::new(0, offset)),
        LABEL_HALF,
    )
}

impl Element for Slider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for SliderLabel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
