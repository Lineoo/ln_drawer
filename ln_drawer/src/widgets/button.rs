use glam::{IVec2, UVec2, Vec4};
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
        renderer::{canvas::Canvas, rrect::RRect, svg::PixImage},
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
    pub modulate: Handle<Val<Vec4>>,
    pub bmp: PixImage,
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

impl ButtonImage {
    pub fn new(bmp: PixImage, modulate: Handle<Val<Vec4>>, size: u32) -> Self {
        ButtonImage {
            transform: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(size)),
            ),
            modulate,
            bmp,
        }
    }
}

impl ToggleButton {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        world.scope("toggle_button");
        world.set_autodeps(Some(handle));
        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);

        let theme = world.single_fetch::<Theme>();

        let frame_color = world.insert(Val::new(match self.selected {
            false => self.theme.idle_color,
            true => self.theme.selected_color,
        }));
        let frame_anim_color = world.build(Animator::new(frame_color, theme.anim_factor));

        world.insert(RRect {
            common: self.common,
            rect: self.rect,
            order: 10,
            color: frame_anim_color,
            radius: theme.roundness,
            width: 0.0,
        });

        world.insert(ToolRectCollider {
            common: self.common,
            target: handle.untyped(),
            rect: self.rect,
            order: 10,
        });

        if let Some(image) = &self.image {
            let icon_rect = world.build(Transform::new(image.transform, self.rect));
            world.insert(Canvas {
                common: self.common,
                rect: icon_rect,
                order: 11,
                modulate: image.modulate,
                data: image.bmp.bytes.clone(),
                data_width: image.bmp.bytes_width,
                data_height: image.bmp.bytes_height,
            });
        }

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
