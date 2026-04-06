use palette::{Srgba, WithAlpha};

use crate::{
    animation::{AnimationDescriptor, AnimationValue},
    render::{rounded::RoundedRectDescriptor, wireframe::WireframeDescriptor},
    widgets::{
        Attach, WidgetButton, WidgetChecked, WidgetDestroyed, WidgetHover, WidgetRectangle,
        WidgetSelect, button::Button, check_button::CheckButton, panel::Panel,
    },
    world::{Element, Handle, World},
};

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    pub color: Srgba,
    pub active_color: Srgba,
    pub press_color: Srgba,
    pub roundness: f32,
    pub press_roundness: f32,
    pub anim_factor: f32,
    pub anim_factor_menu: f32,
    pub pad: i32,
}

impl Default for Luni {
    fn default() -> Self {
        Self {
            color: Srgba::new(0.1, 0.1, 0.1, 0.9),
            active_color: Srgba::new(0.3, 0.3, 0.3, 1.0),
            press_color: Srgba::new(0.2, 0.2, 0.2, 1.0),
            roundness: 5.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
        }
    }
}

impl Element for Luni {}

impl Element for Attach<Button, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let button = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.target).unwrap();

        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: button.rect,
            order: button.order,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        let frame_anim_color = world.build(AnimationDescriptor {
            src: Srgba::new(0.0, 0.0, 0.0, 0.0),
            dst: luni.color,
            factor: luni.anim_factor,
        });

        world.observer(frame_anim_color, move |&AnimationValue(value), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.color = value;
        });

        // dependency

        world.dependency(this, self.widget);
        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        let button = button.handle();
        let luni = luni.handle();

        world.observer(button, move |event: &WidgetHover, world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(button, move |event: &WidgetButton, world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = luni.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = luni.active_color,
            }
        });

        world.observer(button, move |&WidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });
    }
}

impl Element for Attach<CheckButton, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let button = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.target).unwrap();

        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: button.rect,
            order: button.order,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        let frame_anim_color = world.build(AnimationDescriptor {
            src: Srgba::new(0.0, 0.0, 0.0, 0.0),
            dst: luni.color,
            factor: luni.anim_factor,
        });

        let frame_anim_roundness = world.build(AnimationDescriptor {
            src: 0.0,
            dst: luni.roundness,
            factor: luni.anim_factor,
        });

        world.observer(frame_anim_color, move |&AnimationValue(value), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.color = value;
        });
        world.observer(
            frame_anim_roundness,
            move |&AnimationValue(value), world| {
                let mut frame = world.fetch_mut(frame).unwrap();
                frame.desc.shrink = value;
                frame.desc.value = value;
            },
        );

        // dependency

        world.dependency(this, self.widget);
        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);
        world.dependency(frame_anim_roundness, this);

        // behavior

        let button = button.handle();
        let luni = luni.handle();

        world.observer(button, move |event: &WidgetHover, world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(button, move |event: &WidgetButton, world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = luni.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = luni.active_color,
            }
        });

        world.observer(button, move |&WidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });

        world.observer(button, move |&WidgetChecked(checked), world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_roundness = world.fetch_mut(frame_anim_roundness).unwrap();

            match checked {
                true => frame_anim_roundness.dst = luni.press_roundness,
                false => frame_anim_roundness.dst = luni.roundness,
            }
        });
    }
}

impl Element for Attach<Panel, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let panel = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.target).unwrap();

        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: panel.rect,
            order: panel.order,
            color: luni.color,
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        let frame_anim_color = world.build(AnimationDescriptor {
            src: Srgba::new(0.0, 0.0, 0.0, 0.0),
            dst: luni.color,
            factor: luni.anim_factor,
        });

        world.observer(frame_anim_color, move |&AnimationValue(value), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.color = value;
        });

        // dependency

        world.dependency(this, self.widget);
        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        let panel = panel.handle();
        let luni = luni.handle();

        world.observer(panel, move |event: &WidgetHover, world| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(panel, move |&WidgetRectangle(rect), world| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.desc.rect = rect;
        });
    }
}
