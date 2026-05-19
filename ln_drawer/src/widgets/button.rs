use glam::{DVec2, Vec2};
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{AnimationDescriptor, AnimationValue, SimpleAnimationDescriptor},
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        WidgetAnimatedRectangle, WidgetButton, WidgetClick, WidgetEnabled, WidgetHover,
        WidgetRectangle,
    },
};

pub struct Button {
    pub rect: Rectangle,
    pub enabled: bool,
    pub order: isize,
    pub color: Srgba,
    pub active_color: Srgba,
    pub press_color: Srgba,
    pub roundness: f32,
    pub shadow_color: Srgba,
    pub shadow_offset: Vec2,
    pub shadow_blur: f32,
    pub press_roundness: f32,
    pub anim_factor: f32,
    pub anim_factor_menu: f32,
    pub pad: i32,
}

pub struct ButtonDrag {
    pub from: PointerHit,
    pub here: PointerHit,
    pub status: ButtonDragStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonDragStatus {
    Start,
    Dragging,
    End,
}

impl Button {
    fn attach_render(&mut self, world: &World, this: Handle<Self>) {
        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: self.color,
            shadow_color: self.shadow_color,
            shadow_offset: self.shadow_offset,
            shadow_blur: self.shadow_blur,
            shrink: self.roundness,
            value: self.roundness,
            visible: self.enabled,
            order: self.order,
        });

        let frame_rect = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(
                [
                    self.rect.left() as f32,
                    self.rect.down() as f32,
                    self.rect.right() as f32,
                    self.rect.up() as f32,
                ],
                self.anim_factor,
            ),
            widget: frame,
            action: |mut frame, _, rect| {
                frame.desc.rect = Rectangle::new(
                    rect[0].round() as i32,
                    rect[1].round() as i32,
                    rect[2].round() as i32,
                    rect[3].round() as i32,
                );
            },
        });

        let frame_anim_color = world.build(AnimationDescriptor::new(self.color, self.anim_factor));

        world.observer(frame_anim_color, move |&AnimationValue(value), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.color = value;
        });

        // dependency

        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        world.observer(this, move |event: &WidgetHover, world| {
            let this = world.fetch(this).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = this.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = this.color,
            }
        });

        world.observer(this, move |event: &WidgetButton, world| {
            let this = world.fetch(this).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = this.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = this.active_color,
            }
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            let rect = [
                rect.left() as f32,
                rect.down() as f32,
                rect.right() as f32,
                rect.up() as f32,
            ];

            frame_rect.src = rect;
            frame_rect.dst = rect;
        });

        world.observer(this, move |&WidgetAnimatedRectangle(rect), world| {
            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            let rect = [
                rect.left() as f32,
                rect.down() as f32,
                rect.right() as f32,
                rect.up() as f32,
            ];

            frame_rect.dst = rect;
        });

        world.observer(this, move |&WidgetEnabled(enabled), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.visible = enabled;
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: self.order,
            enabled: self.enabled,
        });

        world.insert(Transform {
            value: TransformValue::copy(),
            source: this.untyped(),
            target: collider.untyped(),
        });

        let mut drag_start = None;
        let mut dragging = false;
        world.observer(collider, move |event: &PointerHit, world| {
            const DRAG_DISTANCE: f64 = 0.01;

            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(this, &WidgetButton::ButtonPress);
                    drag_start = Some(*event);
                    dragging = false;
                }
                PointerHitStatus::Moving => {
                    if let Some(start) = drag_start {
                        if DVec2::from_array(event.screen).distance(DVec2::from_array(start.screen))
                            > DRAG_DISTANCE
                            && !dragging
                        {
                            dragging = true;
                            world.trigger(
                                this,
                                &ButtonDrag {
                                    from: start,
                                    here: *event,
                                    status: ButtonDragStatus::Start,
                                },
                            );
                        } else if dragging {
                            world.trigger(
                                this,
                                &ButtonDrag {
                                    from: start,
                                    here: *event,
                                    status: ButtonDragStatus::Dragging,
                                },
                            );
                        }
                    }
                }
                PointerHitStatus::Release => {
                    if !dragging {
                        world.trigger(this, &WidgetClick);
                    } else if let Some(start) = drag_start {
                        world.trigger(
                            this,
                            &ButtonDrag {
                                from: start,
                                here: *event,
                                status: ButtonDragStatus::End,
                            },
                        );
                    }

                    world.trigger(this, &WidgetButton::ButtonRelease);
                    drag_start = None;
                    dragging = false;
                }
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

        world.observer(this, move |&WidgetEnabled(enabled), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = enabled;
        });

        world.dependency(collider, this);
    }

    fn attach_layout(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });

        world.observer(this, move |&WidgetAnimatedRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
        });
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            enabled: true,
            order: 10,
            color: Srgba::new(0.863, 0.863, 0.863, 1.0),
            active_color: Srgba::new(0.808, 0.808, 0.808, 1.0),
            press_color: Srgba::new(0.737, 0.737, 0.737, 1.0),
            roundness: 5.0,
            shadow_color: palette::Srgba::new(0.0, 0.0, 0.0, 0.5),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 10.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
        }
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_layout(world, this);
        self.attach_render(world, this);
        self.attach_pointer(world, this);

        world.queue_trigger(this, WidgetRectangle(self.rect));
        world.queue_trigger(this, WidgetEnabled(self.enabled));
    }
}
