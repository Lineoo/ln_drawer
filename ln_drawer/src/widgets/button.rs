use glam::Vec4;
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        anim::Animator,
        transform::{Transform, TransformValue},
        val::{Val, ValChanged},
    },
    measures::{FI64Ext, Rectangle},
    theme::Theme,
    tools::{
        collider::ToolRectCollider,
        pointer::{PointerHit, PointerHitStatus, PointerHover, PointerHoverStatus},
    },
    widgets::{
        WidgetHover,
        common::WidgetCommon,
        renderer::{
            canvas::{Canvas, SetCanvasColor},
            rrect::{RRect, SetRRectColor},
        },
    },
};

pub struct ToggleButton {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub theme: ToggleButtonTheme,
    pub image: Option<ButtonImage>,
    pub selected: bool,
    pub hovering: bool,
}

pub struct ToggleButtonTheme {
    pub idle_color: Vec4,
    pub hover_color: Vec4,
    pub press_color: Vec4,
    pub selected_color: Vec4,
}

#[derive(Clone)]
pub struct ButtonImage {
    pub transform: TransformValue,
    pub bytes: Vec<u8>,
    pub bytes_width: u32,
    pub bytes_height: u32,
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

pub struct SetButtonIconColor(pub Vec4);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonDragStatus {
    Start,
    Dragging,
    End,
}

impl ToggleButton {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        world.scope("toggle_button");
        world.set_autodeps(Some(handle));
        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);

        let theme = world.single_fetch::<Theme>();

        let frame = world.insert(RRect {
            rect: self.rect,
            order: 10,
            color: match self.selected {
                false => self.theme.idle_color,
                true => self.theme.selected_color,
            },
            radius: theme.roundness,
            width: 0.0,
            visible: self.common.visible,
            camera: self.common.camera,
        });

        let frame_color = world.insert(Val::new(match self.selected {
            false => self.theme.idle_color,
            true => self.theme.selected_color,
        }));
        let frame_anim_color = world.build(Animator::new(frame_color, theme.anim_factor));
        world.observer(frame_anim_color, move |ValChanged, world| {
            let color = world.fetch(frame_anim_color).val;
            world.queue_trigger(frame, SetRRectColor(color));
        });

        world.insert(ToolRectCollider {
            common: self.common,
            target: handle.untyped(),
            rect: self.rect,
            order: 10,
        });

        let canvas = if let Some(image) = &self.image {
            let icon_rect = world.build(Transform::new(image.transform, self.rect));
            let canvas = world.insert(Canvas {
                rect: icon_rect,
                order: 11,
                visible: self.common.visible,
                camera: self.common.camera,
                color: theme.distinct,
                data: image.bytes.clone(),
                data_width: image.bytes_width,
                data_height: image.bytes_height,
            });
            Some(canvas)
        } else {
            None
        };

        world.observer(handle, move |&SetButtonSelected(selected), world| {
            let mut this = world.fetch_mut(handle);
            this.selected = selected;
            let color = if selected {
                this.theme.selected_color
            } else if this.hovering {
                this.theme.hover_color
            } else {
                this.theme.idle_color
            };
            let mut frame_color = world.fetch_mut(frame_color);
            frame_color.val = color;
            world.queue_trigger(frame_color.handle(), ValChanged);
        });

        world.observer(handle, move |&SetButtonIconColor(color), world| {
            if let Some(canvas) = canvas {
                world.queue_trigger(canvas, SetCanvasColor(color));
            }
        });

        let mut drag_start = None;
        let mut dragging = false;
        world.observer(handle, move |event: &PointerHit, world| {
            const DRAG_DISTANCE: f64 = 30.0;

            match event.status {
                PointerHitStatus::Press => {
                    world.trigger(handle, &ButtonAction::Press);
                    drag_start = Some(*event);
                    dragging = false;
                }
                PointerHitStatus::Moving => {
                    if let Some(start) = drag_start {
                        if (event.position - start.position).q32_as_f64().length() > DRAG_DISTANCE
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
                        let this = world.fetch(handle);
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

        world.observer(handle, move |event: &PointerHover, world| {
            let mut this = world.fetch_mut(handle);
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
            let this = world.fetch(handle);
            if this.selected {
                return;
            }

            let color = if let WidgetHover::Enter = event {
                this.theme.hover_color
            } else {
                this.theme.idle_color
            };
            let mut frame_color = world.fetch_mut(frame_color);
            frame_color.val = color;
            world.queue_trigger(frame_color.handle(), ValChanged);
        });

        world.observer(handle, move |event: &ButtonAction, world| {
            let this = world.fetch(handle);
            if this.selected {
                return;
            }

            let color = if let ButtonAction::Press = event {
                this.theme.press_color
            } else if this.hovering {
                this.theme.hover_color
            } else {
                this.theme.idle_color
            };
            let mut frame_color = world.fetch_mut(frame_color);
            frame_color.val = color;
            world.queue_trigger(frame_color.handle(), ValChanged);
        });
    }
}

impl Element for ToggleButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
