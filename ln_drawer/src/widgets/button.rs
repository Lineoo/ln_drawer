use std::sync::Arc;

use image::DynamicImage;
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    animation::{AnimationDescriptor, SetAnimationDst, SimpleAnimationDescriptor},
    layout::transform::TransformValue,
    measures::Rectangle,
    theme::Theme,
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetHover,
        renderer::{
            canvas::{Canvas, SetCanvasColor},
            rrect::{RRect, SetRRectColor},
        },
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

#[expect(unused)]
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

pub struct SetButtonIconColor(pub Srgba);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonDragStatus {
    Start,
    Dragging,
    End,
}

impl ToggleButton {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();

        let frame = world.insert(RRect {
            rect: self.rect,
            order: 10,
            color: match self.selected {
                false => self.theme.idle_color,
                true => self.theme.selected_color,
            },
            radius: theme.roundness,
            width: 0.0,
            enabled: self.visible,
        });

        let frame_anim_color = world.build(SimpleAnimationDescriptor {
            animation: AnimationDescriptor::new(theme.primary_color, theme.anim_factor),
            widget: frame,
            action: move |_, world, color| {
                world.queue_trigger(frame, SetRRectColor(color));
            },
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 10,
            enabled: self.visible,
        });

        let canvas = if let Some(image) = &self.image {
            let data = image.bytes.to_rgba8();
            let canvas = world.insert(Canvas {
                data_width: data.width(),
                data_height: data.height(),
                rect: image.transform.compute(self.rect),
                order: 11,
                visible: self.visible,
                data: data.into_raw(),
                color: theme.symbolic_color,
            });
            Some(canvas)
        } else {
            None
        };

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
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            world.queue_trigger(frame, SetWidgetRectangle(rect));
            collider.rect = rect;

            if let Some(canvas) = canvas
                && let Some(image) = &this.image
            {
                world.queue_trigger(canvas, SetWidgetRectangle(image.transform.compute(rect)));
            }
        });

        world.observer(handle, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.visible = visible;
            world.queue_trigger(frame, SetWidgetVisible(visible));
            collider.enabled = visible;

            if let Some(canvas) = canvas {
                world.queue_trigger(canvas, SetWidgetVisible(visible));
            }
        });

        world.observer(handle, move |&SetButtonIconColor(color), world| {
            if let Some(canvas) = canvas {
                world.queue_trigger(canvas, SetCanvasColor(color));
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
                        if event.pointer.screen.distance(start.pointer.screen) > DRAG_DISTANCE
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
    }
}

impl Element for ToggleButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
