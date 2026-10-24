use glam::{IVec2, UVec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        anim::Animator,
        packer::Packer,
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
    pub selected: Handle<Val<bool>>,
    pub hovering: Handle<Val<bool>>,
}

pub struct ToggleButtonTheme {
    pub idle_color: Handle<Val<Vec4>>,
    pub hover_color: Handle<Val<Vec4>>,
    pub press_color: Handle<Val<Vec4>>,
    pub selected_color: Handle<Val<Vec4>>,
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

        let selected = self.selected;
        let hovering = self.hovering;
        let idle_color = self.theme.idle_color;
        let hover_color = self.theme.hover_color;
        let press_color = self.theme.press_color;
        let selected_color = self.theme.selected_color;

        let pressed = world.insert(Val::new(false));
        let frame_color_anim = Packer::derive(world, move |world| {
            let selected_val = world.fetch(selected).val;
            let hovering_val = world.fetch(hovering).val;
            let pressed_val = world.fetch(pressed).val;
            if selected_val {
                world.fetch(selected_color).val
            } else if pressed_val {
                world.fetch(press_color).val
            } else if hovering_val {
                world.fetch(hover_color).val
            } else {
                world.fetch(idle_color).val
            }
        })
        .listen(selected)
        .listen(hovering)
        .listen(pressed)
        .listen(idle_color)
        .listen(hover_color)
        .listen(press_color)
        .listen(selected_color)
        .finish();
        let frame_color = world.build(Animator::new(frame_color_anim, theme.anim_factor));

        world.insert(RRect {
            common: self.common,
            rect: self.rect,
            order: 10,
            color: frame_color,
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

        world.observer(handle, move |&SetButtonSelected(value), world| {
            world.fetch_mut(selected).val = value;
            world.queue_trigger(selected, ValChanged);
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
                        let selected_val = world.fetch(selected).val;
                        world.queue_trigger(handle, ButtonClick);
                        world.queue_trigger(handle, ButtonSelected(!selected_val));
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
            let hover = match event.status {
                PointerHoverStatus::Enter => true,
                PointerHoverStatus::Leave => false,
                _ => return,
            };
            world.fetch_mut(hovering).val = hover;
            world.queue_trigger(hovering, ValChanged);
            world.queue_trigger(
                handle,
                match hover {
                    true => WidgetHover::Enter,
                    false => WidgetHover::Leave,
                },
            );
        });

        world.observer(handle, move |event: &ButtonAction, world| {
            let press = matches!(event, ButtonAction::Press);
            world.fetch_mut(pressed).val = press;
            world.queue_trigger(pressed, ValChanged);
        });
    }
}

impl Element for ToggleButton {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
