use cosmic_text::{Align, Metrics};
use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        anim::{Animator, AnimatorSnap},
        packer::Packer,
        val::{Val, ValChanged},
    },
    measures::{Axis, FI64Ext, IRect},
    theme::{Theme, ThemeVal},
    tools::{
        collider::ToolRectCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        WidgetHover,
        common::WidgetCommon,
        renderer::{
            rrect::RRect,
            text::{SetText, Text, TextAttr},
        },
    },
};

pub struct SliderValue(pub f32);

pub struct Slider {
    pub common: WidgetCommon,
    pub value: Handle<Val<f32>>,
    pub axis: Axis,
    pub rect: Handle<Val<IRect>>,
    pub pressed: bool,
}

pub struct SliderLabel {
    pub text: String,
    pub clockwise: bool,
    pub source: Handle<Slider>,
    pub hover: bool,
    pub common: WidgetCommon,
}

impl Slider {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        world.scope("slider");
        world.set_autodeps(Some(handle));
        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);

        let theme = world.single_fetch::<Theme>();
        let theme_val = world.single_fetch::<ThemeVal>();
        let rect = world.fetch(self.rect).val;
        let value = world.fetch(self.value).val;

        let position = into_position(rect, self.axis, value);
        let back_rect_anim = world.insert(Val::new(back_rect(rect, self.axis)));
        let back_rect_val = world.build(Animator::new(back_rect_anim, theme.anim_factor));
        let front_rect_val = world.insert(Val::new(front_rect(rect, self.axis, position)));
        let knob_rect_val = world.insert(Val::new(knob_rect(rect, self.axis, position)));
        let knob_split_rect_val =
            world.insert(Val::new(knob_split_rect(rect, self.axis, position)));
        let pressed = world.insert(Val::new(false));

        let knob_color_anim = Packer::derive(world, move |world| {
            let theme = world.single_fetch::<ThemeVal>();
            let pressed_val = world.fetch(pressed).val;
            match pressed_val {
                true => world.fetch(theme.gray1).val,
                false => world.fetch(theme.gray0).val,
            }
        })
        .listen(pressed)
        .listen(theme_val.gray0)
        .listen(theme_val.gray1)
        .finish();
        let knob_color_val = world.build(Animator::new(knob_color_anim, theme.anim_factor));

        let knob_split_color_anim = Packer::derive(world, move |world| {
            let theme = world.single_fetch::<ThemeVal>();
            let pressed_val = world.fetch(pressed).val;
            match pressed_val {
                true => world.fetch(theme.theme).val,
                false => world.fetch(theme.gray1).val,
            }
        })
        .listen(pressed)
        .listen(theme_val.gray1)
        .listen(theme_val.theme)
        .finish();
        let knob_split_color_val =
            world.build(Animator::new(knob_split_color_anim, theme.anim_factor));

        world.insert(RRect {
            common: self.common,
            rect: back_rect_val,
            order: 20,
            color: theme_val.gray2,
            radius: theme.roundness,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: front_rect_val,
            order: 21,
            color: theme_val.theme,
            radius: theme.roundness,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: knob_rect_val,
            order: 22,
            color: knob_color_val,
            radius: theme.roundness,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: knob_split_rect_val,
            order: 23,
            color: knob_split_color_val,
            radius: 2.0,
            width: 0.0,
        });

        world.insert(ToolRectCollider {
            common: self.common,
            target: handle.untyped(),
            rect: self.rect,
            order: 10,
        });

        world.observer(handle, move |event: &PointerHover, world| {
            if let PointerHoverStatus::Enter = event.status {
                world.queue_trigger(handle, WidgetHover::Enter);
            } else if let PointerHoverStatus::Leave = event.status {
                world.queue_trigger(handle, WidgetHover::Leave);
            }
        });

        world.observer(handle, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(handle);
            let rect = world.fetch(this.rect).val;

            let position = match this.axis.is_vertical() {
                true => event.position.q32_round().y,
                false => event.position.q32_round().x,
            };

            let value = from_position_pressed(rect, this.axis, position);
            world.queue_trigger(handle, SliderValue(value));

            if let PointerHitStatus::Press = event.status {
                this.pressed = true;
                let mut back = world.fetch_mut(back_rect_anim);
                let mut pressed = world.fetch_mut(pressed);
                back.val = back_rect_pressed(rect, this.axis);
                pressed.val = true;
                world.queue_trigger(back.handle(), ValChanged);
                world.queue_trigger(pressed.handle(), ValChanged);
            } else if let PointerHitStatus::Release = event.status {
                this.pressed = false;
                let mut back = world.fetch_mut(back_rect_anim);
                let mut pressed = world.fetch_mut(pressed);
                back.val = back_rect(rect, this.axis);
                pressed.val = false;
                world.queue_trigger(back.handle(), ValChanged);
                world.queue_trigger(pressed.handle(), ValChanged);
            }
        });

        world.observer(self.value, move |&ValChanged, world| {
            let this = world.fetch(handle);
            let value = world.fetch(this.value).val;
            let rect = world.fetch(this.rect).val;
            let mut front = world.fetch_mut(front_rect_val);
            let mut knob = world.fetch_mut(knob_rect_val);
            let mut knob_split = world.fetch_mut(knob_split_rect_val);

            let position = match this.pressed {
                true => into_position_pressed(rect, this.axis, value),
                false => into_position(rect, this.axis, value),
            };

            front.val = front_rect(rect, this.axis, position);
            knob_split.val = knob_split_rect(rect, this.axis, position);
            knob.val = match this.pressed {
                true => knob_rect_pressed(rect, this.axis, position),
                false => knob_rect(rect, this.axis, position),
            };

            world.queue_trigger(front.handle(), ValChanged);
            world.queue_trigger(knob_split.handle(), ValChanged);
            world.queue_trigger(knob.handle(), ValChanged);
        });

        world.observer(self.rect, move |ValChanged, world| {
            let this = world.fetch(handle);
            let value = world.fetch(this.value).val;
            let rect = world.fetch(this.rect).val;
            let mut front = world.fetch_mut(front_rect_val);
            let mut knob = world.fetch_mut(knob_rect_val);
            let mut knob_split = world.fetch_mut(knob_split_rect_val);
            let mut back = world.fetch_mut(back_rect_anim);

            let back_rect = back_rect(rect, this.axis);
            back.val = back_rect;

            let position = into_position(rect, this.axis, value);
            front.val = front_rect(rect, this.axis, position);
            knob_split.val = knob_split_rect(rect, this.axis, position);
            knob.val = match this.pressed {
                true => knob_rect_pressed(rect, this.axis, position),
                false => knob_rect(rect, this.axis, position),
            };

            world.queue_trigger(back.handle(), ValChanged);
            world.queue_trigger(back_rect_val, AnimatorSnap);
            world.queue_trigger(front.handle(), ValChanged);
            world.queue_trigger(knob_split.handle(), ValChanged);
            world.queue_trigger(knob.handle(), ValChanged);
        });
    }
}

impl SliderLabel {
    fn init(&self, world: &World, this: Handle<Self>) {
        world.scope("slider_label");
        world.set_autodeps(Some(this));

        let source = world.fetch(self.source);
        let theme = world.single_fetch::<ThemeVal>();
        let rect = world.fetch(source.rect).val;
        let value = world.fetch(source.value).val;

        let position = into_position(rect, source.axis, value);
        let label_rect = world.insert(Val::new(label_back_rect(
            rect,
            source.axis,
            self.clockwise,
            position,
        )));
        let label_visible = world.insert(Val::new(self.hover));
        let common = WidgetCommon {
            camera: self.common.camera,
            visible: label_visible,
        };

        world.insert(RRect {
            common,
            rect: label_rect,
            order: 100,
            color: theme.gray0,
            radius: LABEL_HALF.y as f32,
            width: 0.0,
        });

        let label = world.insert(Text {
            text: self.text.clone(),
            rect: label_rect,
            attr: TextAttr {
                metrics: Metrics::new(12., 2. * LABEL_HALF.y as f32),
                align: Align::Center,
                ..Default::default()
            },
            order: 150,
            common,
            modulate: theme.distinct,
            outdated: false,
            canvas_outdated: false,
        });

        world.observer(self.source, move |event: &WidgetHover, world| {
            let mut this = world.fetch_mut(this);
            let visible = world.fetch(this.common.visible).val;
            let mut label_visible = world.fetch_mut(label_visible);
            this.hover = match event {
                WidgetHover::Enter => true,
                WidgetHover::Leave => false,
            };
            label_visible.val = this.hover && visible;
            world.queue_trigger(label_visible.handle(), ValChanged);
        });

        world.observer(source.rect, move |ValChanged, world| {
            let this = world.fetch(this);
            let source = world.fetch(this.source);
            let rect = world.fetch(source.rect).val;
            let value = world.fetch(source.value).val;
            let mut label_rect = world.fetch_mut(label_rect);
            let position = into_position_pressed(rect, source.axis, value);
            label_rect.val = label_back_rect(rect, source.axis, this.clockwise, position);
            world.queue_trigger(label_rect.handle(), ValChanged);
        });

        world.observer(self.common.visible, move |ValChanged, world| {
            let this = world.fetch(this);
            let visible = world.fetch(this.common.visible).val;
            let mut label_visible = world.fetch_mut(label_visible);
            label_visible.val = this.hover && visible;
            world.queue_trigger(label_visible.handle(), ValChanged);
        });

        world.observer(source.value, move |ValChanged, world| {
            let this = world.fetch(this);
            let source = world.fetch(this.source);
            let rect = world.fetch(source.rect).val;
            let value = world.fetch(source.value).val;
            let mut label_rect = world.fetch_mut(label_rect);
            let position = into_position_pressed(rect, source.axis, value);
            label_rect.val = label_back_rect(rect, source.axis, this.clockwise, position);
            world.queue_trigger(label_rect.handle(), ValChanged);
        });

        world.observer(this, move |val: &SetText, world| {
            world.trigger(label, val);
        });
    }
}

const BAR_EX: i32 = 5;
const BAR_HW: i32 = 6;
const KNOB_SIZE: UVec2 = UVec2::new(8, 16);
const KNOB_SIZE_PRESSED: UVec2 = UVec2::new(7, 16);
const KNOB_SPLIT_SIZE: UVec2 = UVec2::new(5, 2);
const LABEL_GAP: i32 = 2;
const LABEL_HALF: UVec2 = UVec2::new(30, 12);

fn back_rect(rect: IRect, axis: Axis) -> IRect {
    front_rect(rect, axis, rect.axis_end(axis))
}

fn back_rect_pressed(rect: IRect, axis: Axis) -> IRect {
    back_rect(rect, axis).expand(BAR_EX)
}

fn front_rect(rect: IRect, axis: Axis, position: i32) -> IRect {
    IRect::axis_new(
        rect.axis_start(axis),
        rect.axis_center(axis.rotate()) - BAR_HW * axis.sign(),
        position,
        rect.axis_center(axis.rotate()) + BAR_HW * axis.sign(),
        axis,
    )
}

fn knob_rect(rect: IRect, axis: Axis, position: i32) -> IRect {
    IRect::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SIZE),
    )
}

fn knob_rect_pressed(rect: IRect, axis: Axis, position: i32) -> IRect {
    IRect::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SIZE_PRESSED),
    )
}

fn knob_split_rect(rect: IRect, axis: Axis, position: i32) -> IRect {
    IRect::new_half(
        axis.vertical_ivec2(IVec2::new(rect.axis_center(axis.rotate()), position)),
        axis.vertical_uvec2(KNOB_SPLIT_SIZE),
    )
}

fn into_position(rect: IRect, axis: Axis, value: f32) -> i32 {
    let (knob_max, knob_min) = knob_limit(rect, axis);
    (value * (knob_max - knob_min) as f32).round() as i32 + knob_min
}

fn into_position_pressed(rect: IRect, axis: Axis, value: f32) -> i32 {
    let (knob_max, knob_min) = knob_limit_pressed(rect, axis);
    (value * (knob_max - knob_min) as f32).round() as i32 + knob_min
}

fn from_position_pressed(rect: IRect, axis: Axis, position: i32) -> f32 {
    let (knob_max, knob_min) = knob_limit_pressed(rect, axis);
    ((position - knob_min) as f32 / (knob_max - knob_min) as f32).clamp(0.0, 1.0)
}

fn knob_limit(rect: IRect, axis: Axis) -> (i32, i32) {
    (
        rect.axis_end(axis) - KNOB_SIZE.y as i32 * axis.sign(),
        rect.axis_start(axis) + KNOB_SIZE.y as i32 * axis.sign(),
    )
}

fn knob_limit_pressed(rect: IRect, axis: Axis) -> (i32, i32) {
    (
        rect.axis_end(axis) - KNOB_SIZE_PRESSED.y as i32 * axis.sign(),
        rect.axis_start(axis) + KNOB_SIZE_PRESSED.y as i32 * axis.sign(),
    )
}

fn label_back_rect(rect: IRect, axis: Axis, clockwise: bool, position: i32) -> IRect {
    let raxis = match clockwise {
        true => axis.rotate(),
        false => axis.rotate_rev(),
    };
    let offset = BAR_HW + BAR_EX + LABEL_GAP + axis.horizontal_uvec2(LABEL_HALF).y as i32;
    IRect::new_half(
        axis.horizontal_ivec2(IVec2::new(position, rect.axis_center(axis.rotate())))
            + raxis.ivec2() * offset,
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
