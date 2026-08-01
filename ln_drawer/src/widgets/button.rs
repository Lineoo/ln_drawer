use glam::{DVec2, Vec2};
use ln_world::{Descriptor, Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{
        AnimationDescriptor, AnimationType, AnimationValue, DirectAnimation, SetAnimationDst,
        SimpleAnimationDescriptor,
    },
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::{canvas::CanvasDescriptor, rounded::RoundedRectDescriptor},
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        ButtonAction, ButtonClick, SetWidgetRectangle, SetWidgetVisible, WidgetHover,
        WidgetRectangle, WidgetVisible,
    },
};

pub struct Button {
    pub rect: Rectangle,
    pub rect_transition: bool,
    pub enabled: bool,
    pub attach_pointer: bool,
    pub checked: bool,
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
    pub image: Option<ButtonImage>,
}

#[derive(Clone, Copy)]
pub struct ButtonImage {
    pub transform: TransformValue,
    pub bytes: &'static [u8],
}

pub struct ButtonDrag {
    pub from: PointerHit,
    pub here: PointerHit,
    pub status: ButtonDragStatus,
}

pub struct ButtonSelected(pub bool);
pub struct SetButtonSelected(pub bool);
pub struct SetButtonColor(pub Srgba);
pub struct SetButtonAnim {
    pub src: Rectangle,
    pub dst: Rectangle,
    pub hidden_after_finished: bool,
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
            vertex_extend: 20,
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
            action: move |_, world, rect| {
                world.queue_trigger(
                    this,
                    SetWidgetRectangle(Rectangle::new(
                        rect[0].round() as i32,
                        rect[1].round() as i32,
                        rect[2].round() as i32,
                        rect[3].round() as i32,
                    )),
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

        world.observer(this, move |&SetButtonSelected(checked), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.checked = checked;
            match checked {
                true => world.trigger(frame_anim_color, &SetAnimationDst(this.press_color)),
                false => world.trigger(frame_anim_color, &SetAnimationDst(this.color)),
            };
        });

        world.observer(this, move |event: &WidgetHover, world| {
            let this = world.fetch(this).unwrap();
            if this.checked {
                return;
            }
            match event {
                WidgetHover::Enter => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.active_color))
                }
                WidgetHover::Leave => world.trigger(frame_anim_color, &SetAnimationDst(this.color)),
            };
        });

        world.observer(this, move |&SetButtonColor(color), world| {
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            frame_anim_color.src = color.into_storage();
            frame_anim_color.dst = color.into_storage();
        });

        world.observer(this, move |event: &ButtonAction, world| {
            let this = world.fetch(this).unwrap();
            if this.checked {
                return;
            }
            match event {
                ButtonAction::Press => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.press_color))
                }
                ButtonAction::Release => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.active_color))
                }
            };
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });

        world.observer(this, move |anim: &SetButtonAnim, world| {
            let this = world.fetch(this).unwrap();
            if !this.rect_transition {
                return;
            }

            let mut frame_rect = world.fetch_mut(frame_rect).unwrap();
            let src = [
                anim.src.left() as f32,
                anim.src.down() as f32,
                anim.src.right() as f32,
                anim.src.up() as f32,
            ];
            let dst = [
                anim.dst.left() as f32,
                anim.dst.down() as f32,
                anim.dst.right() as f32,
                anim.dst.up() as f32,
            ];

            frame_rect.src = src;
            frame_rect.dst = dst;
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
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
                    world.trigger(this, &ButtonAction::Press);
                    drag_start = Some(*event);
                    dragging = false;
                }
                PointerHitStatus::Moving => {
                    if let Some(start) = drag_start {
                        if DVec2::from_array(event.pointer.screen)
                            .distance(DVec2::from_array(start.pointer.screen))
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
                        world.trigger(this, &ButtonClick);
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

                    world.trigger(this, &ButtonAction::Release);
                    drag_start = None;
                    dragging = false;
                }
            }
        });

        world.observer(collider, move |event: &PointerHover, world| {
            match event.status {
                PointerHoverStatus::Enter => {
                    world.trigger(this, &WidgetHover::Enter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(this, &WidgetHover::Leave);
                }
                _ => {}
            }
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut collider = world.fetch_mut(collider).unwrap();
            collider.enabled = enabled;
        });

        world.dependency(collider, this);
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            rect_transition: true,
            enabled: true,
            attach_pointer: true,
            checked: false,
            order: 10,
            color: Srgba::new(0.949, 0.949, 0.949, 1.0),
            active_color: Srgba::new(0.898, 0.898, 0.898, 1.0),
            press_color: Srgba::new(0.722, 0.722, 0.722, 1.0),
            roundness: 5.0,
            shadow_color: palette::Srgba::new(0.0, 0.0, 0.0, 0.5),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
            image: None,
        }
    }
}

impl Element for Button {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_render(world, this);
        if self.attach_pointer {
            self.attach_pointer(world, this);
        }

        if let Some(image) = self.image
            && let Ok(data) = image::load_from_memory(image.bytes)
        {
            let data = data.into_rgba8();
            let canvas = world.build(CanvasDescriptor {
                data_width: data.width(),
                data_height: data.height(),
                rect: image.transform.compute(self.rect),
                order: self.order + 1,
                visible: self.enabled,
                data: Some(data.into_raw()),
            });

            world.observer(this, move |&SetWidgetRectangle(rect), world| {
                let mut canvas = world.fetch_mut(canvas).unwrap();
                canvas.rect = image.transform.compute(rect);
            });
        }

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
            world.queue_trigger(this.handle(), WidgetRectangle(rect));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.enabled = enabled;
            world.queue_trigger(this.handle(), WidgetVisible(enabled));
        });

        world.queue_trigger(this, SetWidgetRectangle(self.rect));
        world.queue_trigger(this, SetWidgetVisible(self.enabled));
        world.queue_trigger(this, SetButtonSelected(self.checked));
    }
}

pub struct ToggleButton {
    pub rect: Rectangle,
    pub theme: ToggleButtonTheme,
    pub selected: bool,
    pub image: Option<ButtonImage>,
    pub visible: bool,
}

pub struct ToggleButtonTheme {
    pub idle_color: Srgba,
    pub hover_color: Srgba,
    pub press_color: Srgba,
    pub selected_color: Srgba,
}

impl ToggleButton {
    pub fn build(self, world: &World) -> Handle<ToggleButton> {
        let theme = world.single_fetch::<Theme>().unwrap();

        let frame = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: self.theme.idle_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: self.visible,
            order: 10,
        });

        let frame_anim_color = world.build(DirectAnimation {
            init: theme.primary_color,
            factor: theme.anim_factor,
            widget: frame,
            access: |frame| &mut frame.desc.color,
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 10,
            enabled: self.visible,
        });

        let canvas = if let Some(image) = self.image
            && let Ok(data) = image::load_from_memory(image.bytes)
        {
            let data = data.into_rgba8();
            let canvas = world.build(CanvasDescriptor {
                data_width: data.width(),
                data_height: data.height(),
                rect: image.transform.compute(self.rect),
                order: 11,
                visible: self.visible,
                data: Some(data.into_raw()),
            });
            Some(canvas)
        } else {
            None
        };

        let handle = world.insert(self);

        world.observer(handle, move |&SetButtonSelected(selected), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            this.selected = selected;
            match selected {
                true => world.trigger(
                    frame_anim_color,
                    &SetAnimationDst(this.theme.selected_color),
                ),
                false => world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color)),
            };
        });

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            frame.desc.rect = rect;
            collider.rect = rect;

            if let Some(canvas) = canvas
                && let Some(image) = &this.image
            {
                let mut canvas = world.fetch_mut(canvas).unwrap();
                canvas.rect = image.transform.compute(rect);
            }
        });

        world.observer(handle, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = visible;
            frame.desc.visible = visible;
            collider.enabled = visible;

            if let Some(canvas) = canvas {
                let mut canvas = world.fetch_mut(canvas).unwrap();
                canvas.visible = visible;
            }
        });

        let mut drag_start = None;
        let mut dragging = false;
        world.observer(collider, move |event: &PointerHit, world| {
            const DRAG_DISTANCE: f64 = 0.01;

            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(handle, &ButtonAction::Press);
                    drag_start = Some(*event);
                    dragging = false;
                }
                PointerHitStatus::Moving => {
                    if let Some(start) = drag_start {
                        if DVec2::from_array(event.pointer.screen)
                            .distance(DVec2::from_array(start.pointer.screen))
                            > DRAG_DISTANCE
                            && !dragging
                        {
                            dragging = true;
                            world.trigger(
                                handle,
                                &ButtonDrag {
                                    from: start,
                                    here: *event,
                                    status: ButtonDragStatus::Start,
                                },
                            );
                        } else if dragging {
                            world.trigger(
                                handle,
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
                        let this = world.fetch(handle).unwrap();
                        world.queue_trigger(handle, ButtonClick);
                        world.queue_trigger(handle, ButtonSelected(!this.selected));
                    } else if let Some(start) = drag_start {
                        world.trigger(
                            handle,
                            &ButtonDrag {
                                from: start,
                                here: *event,
                                status: ButtonDragStatus::End,
                            },
                        );
                    }

                    world.trigger(handle, &ButtonAction::Release);
                    drag_start = None;
                    dragging = false;
                }
            }
        });

        world.observer(collider, move |event: &PointerHover, world| {
            match event.status {
                PointerHoverStatus::Enter => {
                    world.trigger(handle, &WidgetHover::Enter);
                }
                PointerHoverStatus::Leave => {
                    world.trigger(handle, &WidgetHover::Leave);
                }
                _ => {}
            }
        });

        world.observer(handle, move |event: &WidgetHover, world| {
            let this = world.fetch(handle).unwrap();
            if this.selected {
                return;
            }
            match event {
                WidgetHover::Enter => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color))
                }
                WidgetHover::Leave => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.theme.idle_color))
                }
            };
        });

        world.observer(handle, move |event: &ButtonAction, world| {
            let this = world.fetch(handle).unwrap();
            if this.selected {
                return;
            }
            match event {
                ButtonAction::Press => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.theme.press_color))
                }
                ButtonAction::Release => {
                    world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color))
                }
            };
        });

        handle
    }
}

impl Element for ToggleButton {}
impl Descriptor for ToggleButton {
    type Target = Handle<ToggleButton>;
    fn when_build(self, world: &World) -> Self::Target {
        self.build(world)
    }
}
