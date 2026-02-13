use palette::{Mix, Srgba, WithAlpha};

use crate::{
    animation::{AnimationDescriptor, AnimationValue},
    render::rounded::RoundedRectDescriptor,
    widgets::{
        button::Button,
        check_button::CheckButton,
        events::{WidgetButton, WidgetHover, WidgetModified, WidgetSelect},
        menu::Menu,
        panel::Panel,
    },
    world::{Element, Handle, World},
};

/// Trigger this to *try* to attach a headless widget to a specific theme
pub struct Attach<T>(pub Handle<T>);

/// The default theme widgets will attach to.
pub struct Theme(pub Handle);

impl Element for Theme {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |event: &Attach<Button>, world, this| {
            let this = world.fetch(this).unwrap();
            world.trigger(this.0, event);
        });

        world.observer(this, |event: &Attach<CheckButton>, world, this| {
            let this = world.fetch(this).unwrap();
            world.trigger(this.0, event);
        });

        world.observer(this, |event: &Attach<Panel>, world, this| {
            let this = world.fetch(this).unwrap();
            world.trigger(this.0, event);
        });

        world.observer(this, |event: &Attach<Menu>, world, this| {
            let this = world.fetch(this).unwrap();
            world.trigger(this.0, event);
        });
    }
}

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    back_color: Srgba,
    front_color: Srgba,
    roundness: f32,
    anim_factor: f32,
    anim_factor_menu: f32,
    pad: i32,
}

impl Default for Luni {
    fn default() -> Self {
        Self {
            back_color: Srgba::new(0.1, 0.1, 0.1, 0.9),
            front_color: Srgba::new(0.3, 0.3, 0.3, 1.0),
            roundness: 5.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
        }
    }
}

impl Element for Luni {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |&Attach::<Button>(button), world, this| {
            let button = world.fetch(button).unwrap();
            let this = world.fetch(this).unwrap();

            let back_frame = world.build(RoundedRectDescriptor {
                rect: button.rect,
                order: button.order,
                color: this.back_color,
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let front_frame = world.build(RoundedRectDescriptor {
                rect: button.rect,
                order: button.order + 1,
                color: this.front_color.with_alpha(0.0),
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let animation = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 0.0,
                factor: this.anim_factor,
            });

            let start_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 1.0,
                factor: this.anim_factor,
            });

            let this = this.handle();
            world.observer(animation, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();

                front_frame.color = this.front_color.with_alpha(value);
                front_frame.shrink = 5.0 + value * 2.0;
                front_frame.value = (1.0 - value) * 5.0 + value * 2.0;
            });

            world.observer(
                start_anim,
                move |&AnimationValue::<f32>(value), world, _| {
                    let this = world.fetch(this).unwrap();
                    let mut back_frame = world.fetch_mut(back_frame).unwrap();

                    back_frame.color = this.back_color.with_alpha(this.back_color.alpha * value);

                    if value == 1.0 {
                        world.remove(start_anim);
                    }
                },
            );

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());
            world.dependency(animation, button.handle());
            world.dependency(start_anim, button.handle());

            let button = button.handle();
            world.observer(button, move |event: &WidgetHover, world, _| match event {
                WidgetHover::HoverEnter => {
                    let mut animation = world.fetch_mut(animation).unwrap();
                    animation.dst = 1.0;
                }
                WidgetHover::HoverLeave => {
                    let mut animation = world.fetch_mut(animation).unwrap();
                    animation.dst = 0.0;
                }
            });

            world.observer(button, move |event: &WidgetButton, world, _| match event {
                WidgetButton::ButtonPress => {
                    let mut animation = world.fetch_mut(animation).unwrap();
                    animation.dst = 0.5;
                }
                WidgetButton::ButtonRelease => {
                    let mut animation = world.fetch_mut(animation).unwrap();
                    animation.dst = 1.0;
                }
            });

            world.observer(button, move |WidgetModified, world, _| {
                let button = world.fetch(button).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();
                let mut back_frame = world.fetch_mut(back_frame).unwrap();

                front_frame.rect = button.rect;
                front_frame.order = button.order + 1;
                back_frame.rect = button.rect;
                back_frame.order = button.order;
            });
        });

        world.observer(this, |&Attach::<CheckButton>(button), world, this| {
            let button = world.fetch(button).unwrap();
            let this = world.fetch(this).unwrap();

            let back_frame = world.build(RoundedRectDescriptor {
                rect: button.rect,
                order: button.order,
                color: this.back_color,
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let front_frame = world.build(RoundedRectDescriptor {
                rect: button.rect,
                order: button.order + 1,
                color: this.front_color.with_alpha(0.0),
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let front_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 0.0,
                factor: this.anim_factor,
            });

            let back_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 0.0,
                factor: this.anim_factor,
            });

            let start_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 1.0,
                factor: this.anim_factor,
            });

            let this = this.handle();
            world.observer(front_anim, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();

                front_frame.color = this.front_color.with_alpha(value);
                front_frame.shrink = 5.0 + value * 2.0;
                front_frame.value = (1.0 - value) * 5.0 + value * 2.0;
            });

            world.observer(back_anim, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut back_frame = world.fetch_mut(back_frame).unwrap();
                back_frame.color = this.back_color.mix(this.front_color, value);
            });
            world.observer(
                start_anim,
                move |&AnimationValue::<f32>(value), world, _| {
                    let this = world.fetch(this).unwrap();
                    let mut back_frame = world.fetch_mut(back_frame).unwrap();

                    back_frame.color = this.back_color.with_alpha(this.back_color.alpha * value);

                    if value == 1.0 {
                        world.remove(start_anim);
                    }
                },
            );

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());
            world.dependency(front_anim, button.handle());
            world.dependency(back_anim, button.handle());
            world.dependency(start_anim, button.handle());

            let button = button.handle();
            world.observer(button, move |event: &WidgetHover, world, _| match event {
                WidgetHover::HoverEnter => {
                    let mut animation = world.fetch_mut(front_anim).unwrap();
                    animation.dst = 1.0;
                }
                WidgetHover::HoverLeave => {
                    let mut animation = world.fetch_mut(front_anim).unwrap();
                    animation.dst = 0.0;
                }
            });

            world.observer(button, move |event: &WidgetButton, world, _| match event {
                WidgetButton::ButtonPress => {
                    let mut animation = world.fetch_mut(front_anim).unwrap();
                    animation.dst = 0.5;
                }
                WidgetButton::ButtonRelease => {
                    let mut animation = world.fetch_mut(front_anim).unwrap();
                    animation.dst = 1.0;
                }
            });

            world.observer(button, move |WidgetModified, world, button| {
                let button = world.fetch(button).unwrap();
                let mut animation = world.fetch_mut(back_anim).unwrap();
                animation.dst = match button.checked {
                    true => 0.5,
                    false => 0.0,
                };

                let mut front_frame = world.fetch_mut(front_frame).unwrap();
                let mut back_frame = world.fetch_mut(back_frame).unwrap();

                front_frame.rect = button.rect;
                front_frame.order = button.order + 1;
                back_frame.rect = button.rect;
                back_frame.order = button.order;
            });
        });

        world.observer(this, |&Attach::<Panel>(panel), world, this| {
            let panel = world.fetch(panel).unwrap();
            let this = world.fetch(this).unwrap();

            let frame = world.build(RoundedRectDescriptor {
                rect: panel.rect,
                order: panel.order,
                color: this.back_color,
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 0.0,
                factor: this.anim_factor,
            });

            let start_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 1.0,
                factor: this.anim_factor,
            });

            let this = this.handle();
            world.observer(anim, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut back_frame = world.fetch_mut(frame).unwrap();
                back_frame.color = this.back_color.mix(this.front_color, value);
            });

            world.observer(
                start_anim,
                move |&AnimationValue::<f32>(value), world, _| {
                    let this = world.fetch(this).unwrap();
                    let mut back_frame = world.fetch_mut(frame).unwrap();

                    back_frame.color = this.back_color.with_alpha(this.back_color.alpha * value);

                    if value == 1.0 {
                        world.remove(start_anim);
                    }
                },
            );

            world.dependency(frame, panel.handle());
            world.dependency(anim, panel.handle());
            world.dependency(start_anim, panel.handle());

            let panel = panel.handle();
            world.observer(panel, move |event: &WidgetHover, world, _| match event {
                WidgetHover::HoverEnter => {
                    let mut animation = world.fetch_mut(anim).unwrap();
                    animation.dst = 1.0;
                }
                WidgetHover::HoverLeave => {
                    let mut animation = world.fetch_mut(anim).unwrap();
                    animation.dst = 0.0;
                }
            });

            world.observer(panel, move |WidgetModified, world, _| {
                let panel = world.fetch(panel).unwrap();
                let mut frame = world.fetch_mut(frame).unwrap();

                frame.rect = panel.rect;
                frame.order = panel.order;
            });
        });

        world.observer(this, |&Attach::<Menu>(menu), world, this| {
            let menu = world.fetch(menu).unwrap();
            let this = world.fetch(this).unwrap();

            let frame = world.build(RoundedRectDescriptor {
                rect: menu.menu_rect(),
                order: 100,
                color: this.back_color,
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let select_frame = world.build(RoundedRectDescriptor {
                rect: menu.menu_rect(),
                order: 101,
                color: this.front_color.with_alpha(0.0),
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let menu = menu.handle();

            let color_anim = world.build(AnimationDescriptor {
                src: this.back_color.with_alpha(0.0),
                dst: this.back_color,
                factor: this.anim_factor,
            });

            let select_color_anim = world.build(AnimationDescriptor {
                src: this.back_color.with_alpha(0.0),
                dst: this.back_color,
                factor: this.anim_factor_menu,
            });

            let select_rect_anim = world.build(AnimationDescriptor {
                src: 0.0,
                dst: 0.0,
                factor: this.anim_factor_menu,
            });

            world.observer(color_anim, move |&AnimationValue(value), world, _| {
                let mut frame = world.fetch_mut(frame).unwrap();
                frame.color = value;
            });

            world.observer(
                select_color_anim,
                move |&AnimationValue(value), world, _| {
                    let mut select_frame = world.fetch_mut(select_frame).unwrap();
                    select_frame.color = value;
                },
            );

            world.observer(
                select_rect_anim,
                move |&AnimationValue::<f32>(value), world, _| {
                    let mut select_frame = world.fetch_mut(select_frame).unwrap();
                    let menu = world.fetch(menu).unwrap();
                    select_frame.rect = menu.entry_rect(value);
                },
            );

            world.dependency(frame, menu);
            world.dependency(select_frame, menu);
            world.dependency(color_anim, menu);
            world.dependency(select_color_anim, menu);
            world.dependency(select_rect_anim, menu);

            world.observer(menu, move |WidgetModified, world, _| {
                let panel = world.fetch(menu).unwrap();
                let mut frame = world.fetch_mut(frame).unwrap();

                frame.rect = panel.menu_rect();
                frame.order = 100;
            });

            let this = this.handle();
            world.observer(menu, move |event: &WidgetSelect, world, _| match event {
                WidgetSelect(Some(idx)) => {
                    let this = world.fetch(this).unwrap();
                    let mut select_color_anim = world.fetch_mut(select_color_anim).unwrap();
                    let mut select_rect_anim = world.fetch_mut(select_rect_anim).unwrap();
                    select_color_anim.dst = this.front_color;
                    select_rect_anim.dst = *idx as f32;
                }
                WidgetSelect(None) => {
                    let this = world.fetch(this).unwrap();
                    let mut select_color_anim = world.fetch_mut(select_color_anim).unwrap();
                    select_color_anim.dst = this.front_color.with_alpha(0.0);
                }
            });
        });
    }
}
