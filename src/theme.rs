use palette::{Mix, Srgba, WithAlpha};

use crate::{
    animation::{AnimationDescriptor, AnimationValue},
    render::rounded::RoundedRectDescriptor,
    widgets::{button::Button, check_button::CheckButton, events::Interact},
    world::{Element, Handle, World},
};

/// Trigger this to *try* to attach a headless widget to a specific theme
pub struct Attach<T>(pub Handle<T>);

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    back_color: Srgba,
    front_color: Srgba,
    roundness: f32,
    pad: i32,
}

impl Default for Luni {
    fn default() -> Self {
        Self {
            back_color: Srgba::new(0.1, 0.1, 0.1, 0.9),
            front_color: Srgba::new(0.3, 0.3, 0.3, 1.0),
            roundness: 5.0,
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
                rect: button.rect.expand(-this.pad),
                order: button.order + 1,
                color: this.front_color.with_alpha(0.0),
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let animation = world.build(AnimationDescriptor {
                init: 0.0,
                factor: 5.0,
            });

            let this = this.handle();
            world.observer(animation, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();
                let back_frame = world.fetch(back_frame).unwrap();

                front_frame.color = this.front_color.with_alpha(value);
                front_frame.shrink = 5.0 + value * 2.0;
                front_frame.value = (1.0 - value) * 5.0 + value * 2.0;
                front_frame.rect = back_frame.rect;
            });

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());
            world.dependency(animation, button.handle());

            world.observer(
                button.handle(),
                move |interact: &Interact, world, _| match interact {
                    Interact::HoverEnter => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        animation.target(1.0);
                    }
                    Interact::HoverLeave => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        animation.target(0.0);
                    }
                    Interact::ButtonPress => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        animation.target(0.5);
                    }
                    Interact::ButtonRelease => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        animation.target(1.0);
                    }
                    Interact::PropertyChange => {}
                },
            );
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
                rect: button.rect.expand(-this.pad),
                order: button.order + 1,
                color: this.front_color.with_alpha(0.0),
                shrink: this.roundness,
                value: this.roundness,
                ..Default::default()
            });

            let front_anim = world.build(AnimationDescriptor {
                init: 0.0,
                factor: 5.0,
            });

            let back_anim = world.build(AnimationDescriptor {
                init: 0.0,
                factor: 5.0,
            });

            let this = this.handle();
            world.observer(front_anim, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();
                let back_frame = world.fetch(back_frame).unwrap();

                front_frame.color = this.front_color.with_alpha(value);
                front_frame.shrink = 5.0 + value * 2.0;
                front_frame.value = (1.0 - value) * 5.0 + value * 2.0;
                front_frame.rect = back_frame.rect;
            });

            world.observer(back_anim, move |&AnimationValue(value), world, _| {
                let this = world.fetch(this).unwrap();
                let mut back_frame = world.fetch_mut(back_frame).unwrap();
                back_frame.color = this.back_color.mix(this.front_color, value);
            });

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());
            world.dependency(front_anim, button.handle());
            world.dependency(back_anim, button.handle());

            world.observer(
                button.handle(),
                move |interact: &Interact, world, button| match interact {
                    Interact::HoverEnter => {
                        let mut animation = world.fetch_mut(front_anim).unwrap();
                        animation.target(1.0);
                    }
                    Interact::HoverLeave => {
                        let mut animation = world.fetch_mut(front_anim).unwrap();
                        animation.target(0.0);
                    }
                    Interact::ButtonPress => {
                        let mut animation = world.fetch_mut(front_anim).unwrap();
                        animation.target(0.5);
                    }
                    Interact::ButtonRelease => {
                        let mut animation = world.fetch_mut(front_anim).unwrap();
                        animation.target(1.0);
                    }
                    Interact::PropertyChange => {
                        let button = world.fetch(button).unwrap();
                        let mut animation = world.fetch_mut(back_anim).unwrap();
                        animation.target(match button.checked {
                            true => 0.5,
                            false => 0.0,
                        });
                    }
                },
            );
        });
    }
}
