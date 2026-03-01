use palette::{Srgba, WithAlpha};

use crate::{
    animation::{AnimationDescriptor, AnimationValue},
    render::rounded::RoundedRectDescriptor,
    widgets::{
        button::Button,
        check_button::CheckButton,
        events::{
            WidgetButton, WidgetDestroyed, WidgetHover, WidgetModified, WidgetSelect, WidgetSwitch,
        },
        menu::Menu,
        panel::Panel,
    },
    world::{Element, Handle, World},
};

/// Attach a headless widget to a specific theme.
///
/// Create observers on widgets' events
pub struct Attach<T, U> {
    pub widget: Handle<T>,
    pub theme: Handle<U>,
}

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    color: Srgba,
    active_color: Srgba,
    press_color: Srgba,
    roundness: f32,
    press_roundness: f32,
    anim_factor: f32,
    anim_factor_menu: f32,
    pad: i32,
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
        let luni = world.fetch(self.theme).unwrap();

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

        world.observer(frame_anim_color, move |&AnimationValue(value), world, _| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.color = value;
        });

        // dependency

        world.dependency(this, self.widget);
        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        let button = button.handle();
        let luni = luni.handle();

        world.observer(button, move |event: &WidgetHover, world, _| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(button, move |event: &WidgetButton, world, _| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = luni.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = luni.active_color,
            }
        });

        world.observer(button, move |WidgetModified, world, _| {
            let button = world.fetch(button).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();

            frame.rect = button.rect;
            frame.order = button.order;
        });
    }
}

impl Element for Attach<CheckButton, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let button = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.theme).unwrap();

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

        world.observer(frame_anim_color, move |&AnimationValue(value), world, _| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.color = value;
        });
        world.observer(
            frame_anim_roundness,
            move |&AnimationValue(value), world, _| {
                let mut frame = world.fetch_mut(frame).unwrap();
                frame.shrink = value;
                frame.value = value;
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

        world.observer(button, move |event: &WidgetHover, world, _| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(button, move |event: &WidgetButton, world, _| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetButton::ButtonPress => frame_anim_color.dst = luni.press_color,
                WidgetButton::ButtonRelease => frame_anim_color.dst = luni.active_color,
            }
        });

        world.observer(button, move |WidgetModified, world, _| {
            let button = world.fetch(button).unwrap();
            let luni = world.fetch(luni).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();
            let mut frame_anim_roundness = world.fetch_mut(frame_anim_roundness).unwrap();

            frame.rect = button.rect;
            frame.order = button.order;
            match button.checked {
                true => frame_anim_roundness.dst = luni.press_roundness,
                false => frame_anim_roundness.dst = luni.roundness,
            }
        });
    }
}

impl Element for Attach<Panel, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let panel = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.theme).unwrap();

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

        world.observer(frame_anim_color, move |&AnimationValue(value), world, _| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.color = value;
        });

        // dependency

        world.dependency(this, self.widget);
        world.dependency(frame, this);
        world.dependency(frame_anim_color, this);

        // behavior

        let panel = panel.handle();
        let luni = luni.handle();

        world.observer(panel, move |event: &WidgetHover, world, _| {
            let luni = world.fetch(luni).unwrap();
            let mut frame_anim_color = world.fetch_mut(frame_anim_color).unwrap();
            match event {
                WidgetHover::HoverEnter => frame_anim_color.dst = luni.active_color,
                WidgetHover::HoverLeave => frame_anim_color.dst = luni.color,
            }
        });

        world.observer(panel, move |WidgetModified, world, _| {
            let panel = world.fetch(panel).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();

            frame.rect = panel.rect;
            frame.order = panel.order;
        });
    }
}

impl Element for Attach<Menu, Luni> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let menu = world.fetch(self.widget).unwrap();
        let luni = world.fetch(self.theme).unwrap();

        // display

        let frame = world.build(RoundedRectDescriptor {
            rect: menu.menu_rect(),
            order: 100,
            color: luni.color.with_alpha(0.0),
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        let select = world.build(RoundedRectDescriptor {
            rect: menu.entry_rect(0.0),
            order: 101,
            color: luni.active_color.with_alpha(0.0),
            shrink: luni.roundness,
            value: luni.roundness,
            ..Default::default()
        });

        // anim bind

        let frame_anim_alpha = world.build(AnimationDescriptor {
            src: 0.0,
            dst: 1.0,
            factor: luni.anim_factor,
        });

        let select_anim_alpha = world.build(AnimationDescriptor::new(0.0, luni.anim_factor));
        let select_anim_rect = world.build(AnimationDescriptor::new(0.0, luni.anim_factor_menu));

        world.observer(frame_anim_alpha, move |&AnimationValue(value), world, _| {
            let mut frame = world.fetch_mut(frame).unwrap();
            frame.color.alpha = value;
        });

        world.observer(
            select_anim_alpha,
            move |&AnimationValue(value), world, _| {
                let mut select = world.fetch_mut(select).unwrap();
                select.color.alpha = value;
            },
        );

        let menu = menu.handle();
        world.observer(select_anim_rect, move |&AnimationValue(value), world, _| {
            let mut select_frame = world.fetch_mut(select).unwrap();
            let menu = world.fetch(menu).unwrap();
            select_frame.rect = menu.entry_rect(value);
        });

        // dependency

        world.dependency(frame, this);
        world.dependency(select, this);
        world.dependency(select_anim_alpha, this);
        world.dependency(select_anim_rect, menu);

        // behavior

        world.observer(menu, move |WidgetModified, world, _| {
            let menu = world.fetch(menu).unwrap();
            let mut frame = world.fetch_mut(frame).unwrap();

            frame.rect = menu.menu_rect();
            frame.order = 100;
        });

        world.observer(menu, move |event: &WidgetSelect, world, _| match event {
            WidgetSelect(Some(idx)) => {
                let mut select_anim_alpha = world.fetch_mut(select_anim_alpha).unwrap();
                let mut select_anim_rect = world.fetch_mut(select_anim_rect).unwrap();
                select_anim_alpha.dst = 1.0;
                select_anim_rect.dst = *idx as f32;
            }
            WidgetSelect(None) => {
                let mut select_anim_alpha = world.fetch_mut(select_anim_alpha).unwrap();
                select_anim_alpha.dst = 0.0;
            }
        });

        world.observer(menu, move |WidgetDestroyed, world, _| {
            world.observer(
                frame_anim_alpha,
                move |&AnimationValue::<f32>(value), world, _| {
                    if value == 0f32 {
                        world.remove(this);
                    }
                },
            );

            let mut frame_anim_alpha = world.fetch_mut(frame_anim_alpha).unwrap();
            let mut select_anim_alpha = world.fetch_mut(select_anim_alpha).unwrap();
            frame_anim_alpha.dst = 0.0;
            select_anim_alpha.dst = 0.0;
        });
    }
}
