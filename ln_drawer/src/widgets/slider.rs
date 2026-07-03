use glam::{IVec2, UVec2, Vec2};
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{AnimationType, DirectAnimation, SetAnimationDst},
    measures::{FI64Ext, Rectangle},
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHover, PointerHoverStatus},
    },
    widgets::{WidgetHover, WidgetRectangle},
};

/// Related events: [`SetSlider`], [`SliderKnobHover`], [`WidgetHover`]
pub struct VSlider {
    pub x: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

pub struct SetSlider {
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

pub enum SliderKnobHover {
    HoverEnter,
    HoverLeave,
}

impl VSlider {
    pub fn receive_event(slider: Handle<VSlider>, world: &World) {
        world.observer(slider, move |&SetSlider { min, max, value }, world| {
            let mut this = world.fetch_mut(slider).unwrap();
            this.min = min;
            this.max = max;
            this.value = value;
        });

        world.observer(slider, move |&WidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(slider).unwrap();
            this.y_max = rect.up();
            this.y_min = rect.down();
            this.x = rect.horizontal_center();
        });
    }

    pub fn create_renderer(slider: Handle<VSlider>, world: &World) {
        let this = world.fetch(slider).unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let y_knob_max = this.y_max - 23;
        let y_knob_min = this.y_min + 23;

        let back = world.build(RoundedRectDescriptor {
            rect: Rectangle::new(this.x - 10, this.y_min, this.x + 10, this.y_max),
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

        let value = div_height(this.value, this.max, this.min, y_knob_max, y_knob_min);

        let front = world.build(RoundedRectDescriptor {
            rect: Rectangle::new(this.x - 10, this.y_min, this.x + 10, value),
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
            rect: Rectangle::new_half(IVec2::new(this.x, value), UVec2::new(12, 25)),
            color: theme.primary_color,
            shadow_color: theme.shadow_color,
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 10.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 20,
            visible: true,
            order: 2,
        });

        let knob_split = world.build(RoundedRectDescriptor {
            rect: Rectangle::new_half(IVec2::new(this.x, value), UVec2::new(8, 2)),
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
            init: Rectangle::new(this.x - 10, this.y_min, this.x + 10, this.y_max),
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

        world.observer(slider, move |&SetSlider { min, max, value }, world| {
            let this = world.fetch(slider).unwrap();
            let mut front = world.fetch_mut(front).unwrap();
            let mut knob = world.fetch_mut(knob).unwrap();
            let mut knob_split = world.fetch_mut(knob_split).unwrap();

            let y_knob_max = this.y_max - 23;
            let y_knob_min = this.y_min + 23;

            let value = div_height(value, max, min, y_knob_max, y_knob_min);

            front.desc.rect = Rectangle::new(this.x - 10, this.y_min, this.x + 10, value);
            knob.desc.rect = Rectangle::new_half(IVec2::new(this.x, value), UVec2::new(12, 25));
            knob_split.desc.rect = Rectangle::new_half(IVec2::new(this.x, value), UVec2::new(8, 2));
        });

        world.observer(slider, move |&WidgetRectangle(rect), world| {
            let this = world.fetch(slider).unwrap();
            let mut back_rect_anim = world.fetch_mut(back_rect_anim).unwrap();
            let mut front = world.fetch_mut(front).unwrap();
            let mut knob = world.fetch_mut(knob).unwrap();
            let mut knob_split = world.fetch_mut(knob_split).unwrap();

            let y_max = rect.up();
            let y_min = rect.down();
            let x = rect.horizontal_center();

            let y_knob_max = y_max - 23;
            let y_knob_min = y_min + 23;

            let value = div_height(this.value, this.max, this.min, y_knob_max, y_knob_min);

            let back_rect = Rectangle::new(this.x - 10, this.y_min, this.x + 10, this.y_max);
            back_rect_anim.dst = back_rect.into_storage();
            back_rect_anim.src = back_rect.into_storage();

            front.desc.rect = Rectangle::new(x - 10, y_min, x + 10, value);
            knob.desc.rect = Rectangle::new_half(IVec2::new(x, value), UVec2::new(12, 25));
            knob_split.desc.rect = Rectangle::new_half(IVec2::new(x, value), UVec2::new(8, 2));
        });

        world.observer(slider, move |event: &WidgetHover, world| {
            let this = world.fetch(slider).unwrap();

            match event {
                WidgetHover::HoverEnter => {
                    world.trigger(
                        back_rect_anim,
                        &SetAnimationDst(Rectangle::new(
                            this.x - 15,
                            this.y_min - 5,
                            this.x + 15,
                            this.y_max + 5,
                        )),
                    );
                }
                WidgetHover::HoverLeave => {
                    world.trigger(
                        back_rect_anim,
                        &SetAnimationDst(Rectangle::new(
                            this.x - 10,
                            this.y_min,
                            this.x + 10,
                            this.y_max,
                        )),
                    );
                }
            }
        });

        world.observer(slider, move |event: &SliderKnobHover, world| {
            let theme = world.single_fetch::<Theme>().unwrap();

            match event {
                SliderKnobHover::HoverEnter => {
                    world.trigger(knob_color_anim, &SetAnimationDst(theme.secondary_color));
                    world.trigger(
                        knob_split_color_anim,
                        &SetAnimationDst(theme.significant_color),
                    );
                }
                SliderKnobHover::HoverLeave => {
                    world.trigger(knob_color_anim, &SetAnimationDst(theme.primary_color));
                    world.trigger(
                        knob_split_color_anim,
                        &SetAnimationDst(theme.secondary_color),
                    );
                }
            }
        });
    }

    pub fn create_interact(slider: Handle<VSlider>, world: &World) {
        let this = world.fetch(slider).unwrap();

        let collider = world.insert(ToolCollider {
            rect: Rectangle::new(this.x - 15, this.y_min, this.x + 15, this.y_max),
            order: 10,
            enabled: true,
        });

        world.observer(collider, move |event: &PointerHit, world| {
            let this = world.fetch(slider).unwrap();
            let y_knob_max = this.y_max - 23;
            let y_knob_min = this.y_min + 23;
            world.queue_trigger(
                slider,
                SetSlider {
                    max: this.max,
                    min: this.min,
                    value: div_height_rev(
                        event.position.q32_round().y,
                        this.max,
                        this.min,
                        y_knob_max,
                        y_knob_min,
                    ),
                },
            );
        });

        let mut knob_hover = false;
        world.observer(collider, move |event: &PointerHover, world| {
            let this = world.fetch(slider).unwrap();
            let y_knob_max = this.y_max - 23;
            let y_knob_min = this.y_min + 23;

            let value = div_height(this.value, this.max, this.min, y_knob_max, y_knob_min);
            let knob_rect = Rectangle::new_half(IVec2::new(this.x, value), UVec2::new(12, 25));

            let knob_hover_next = knob_rect.contains(event.position.q32_round());
            if knob_hover_next && !knob_hover {
                knob_hover = true;
                world.trigger(slider, &SliderKnobHover::HoverEnter);
            } else if !knob_hover_next && knob_hover {
                knob_hover = false;
                world.trigger(slider, &SliderKnobHover::HoverLeave);
            }

            match event.status {
                PointerHoverStatus::Enter => {
                    world.trigger(slider, &WidgetHover::HoverEnter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(slider, &WidgetHover::HoverLeave);
                    if knob_hover {
                        knob_hover = false;
                        world.trigger(slider, &SliderKnobHover::HoverLeave);
                    }
                }
                _ => {}
            }
        });

        world.observer(slider, move |&WidgetRectangle(rect), world| {
            let mut collider = world.fetch_mut(collider).unwrap();

            let y_max = rect.up();
            let y_min = rect.down();
            let x = rect.horizontal_center();

            collider.rect = Rectangle::new(x - 15, y_min, x + 15, y_max);
        });
    }
}

fn div_height(value: f32, max: f32, min: f32, y_knob_max: i32, y_knob_min: i32) -> i32 {
    ((value - min) / (max - min) * (y_knob_max - y_knob_min) as f32).round() as i32 + y_knob_min
}

fn div_height_rev(y: i32, max: f32, min: f32, y_knob_max: i32, y_knob_min: i32) -> f32 {
    ((y - y_knob_min) as f32 / (y_knob_max - y_knob_min) as f32).clamp(0.0, 1.0) * (max - min) + min
}

impl Element for VSlider {}
