use std::sync::Arc;

use glam::{DVec2, Vec2};
use image::DynamicImage;
use ln_world::{Descriptor, Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{DirectAnimation, SetAnimationDst},
    layout::transform::TransformValue,
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetHover, renderer::canvas::CanvasDescriptor,
    },
};

pub struct ToggleButton {
    pub rect: Rectangle,
    pub theme: ToggleButtonTheme,
    pub image: Option<ButtonImage>,
    pub selected: bool,
    pub visible: bool,
    pub hovering: bool,
}

pub struct ToggleButtonTheme {
    pub idle_color: Srgba,
    pub hover_color: Srgba,
    pub press_color: Srgba,
    pub selected_color: Srgba,
}

#[derive(Clone)]
pub struct ButtonImage {
    pub transform: TransformValue,
    pub bytes: Arc<DynamicImage>,
}

pub struct ButtonDrag {
    pub from: PointerHit,
    pub here: PointerHit,
    pub status: ButtonDragStatus,
}

pub struct ButtonClick;

pub enum ButtonAction {
    Press,
    Release,
}

pub struct ButtonSelected(pub bool);
pub struct SetButtonSelected(pub bool);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonDragStatus {
    Start,
    Dragging,
    End,
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

        let canvas = if let Some(image) = &self.image {
            let data = image.bytes.to_rgba8();
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
            if selected {
                world.trigger(
                    frame_anim_color,
                    &SetAnimationDst(this.theme.selected_color),
                );
            } else if this.hovering {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color));
            } else {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.idle_color));
            }
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
            let mut this = world.fetch_mut(handle).unwrap();
            match event.status {
                PointerHoverStatus::Enter => {
                    this.hovering = true;
                    world.queue_trigger(handle, WidgetHover::Enter);
                }
                PointerHoverStatus::Leave => {
                    this.hovering = false;
                    world.queue_trigger(handle, WidgetHover::Leave);
                }
                _ => {}
            }
        });

        world.observer(handle, move |event: &WidgetHover, world| {
            let this = world.fetch(handle).unwrap();
            if this.selected {
                return;
            }

            if let WidgetHover::Enter = event {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color));
            } else {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.idle_color));
            }
        });

        world.observer(handle, move |event: &ButtonAction, world| {
            let this = world.fetch(handle).unwrap();
            if this.selected {
                return;
            }

            if let ButtonAction::Press = event {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.press_color));
            } else if this.hovering {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.hover_color));
            } else {
                world.trigger(frame_anim_color, &SetAnimationDst(this.theme.idle_color));
            }
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
