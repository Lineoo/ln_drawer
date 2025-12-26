use std::time::Instant;

use palette::{Srgba, WithAlpha};

use crate::{
    lnwin::Lnwindow,
    render::{RedrawPrepare, RenderControl, rounded::RoundedRectDescriptor},
    widgets::{button::Button, events::Interact},
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

struct Animation {
    current: f32,
    target: f32,
    last_update: Instant,
}

impl Animation {
    fn update(&mut self) -> f32 {
        let delta = (Instant::now() - self.last_update).as_secs_f32();
        let dest = self.current + (self.target - self.current).signum() * 10.0 * delta;
        self.last_update = Instant::now();
        self.current = dest.clamp(self.current.min(self.target), self.current.max(self.target));
        self.current
    }
}

impl Element for Animation {}

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

            let control = world.insert(RenderControl {
                visible: true,
                order: 0,
                refreshing: false,
            });

            let animation = world.insert(Animation {
                current: 0.0,
                target: 0.0,
                last_update: Instant::now(),
            });

            let this = this.handle();
            world.observer(control, move |RedrawPrepare, world, control| {
                let this = world.fetch(this).unwrap();
                let mut animation = world.fetch_mut(animation).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();
                let back_frame = world.fetch(back_frame).unwrap();
                let mut control = world.fetch_mut(control).unwrap();

                let factor = animation.update();
                if factor != animation.target {
                    front_frame.color = this.front_color.with_alpha(factor);
                    front_frame.shrink = 5.0 + factor * 2.0;
                    front_frame.value = (1.0 - factor) * 5.0 + factor * 2.0;
                    front_frame.rect = back_frame.rect;
                }

                if control.refreshing {
                    control.refreshing = factor != animation.target
                }
            });

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());
            world.dependency(control, button.handle());

            world.observer(
                button.handle(),
                move |interact: &Interact, world, _| match interact {
                    Interact::HoverEnter => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        let mut control = world.fetch_mut(control).unwrap();
                        animation.target = 1.0;
                        animation.last_update = Instant::now();
                        control.refreshing = true;
                    }
                    Interact::HoverLeave => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        let mut control = world.fetch_mut(control).unwrap();
                        animation.target = 0.0;
                        animation.last_update = Instant::now();
                        control.refreshing = true;
                    }
                    Interact::ButtonPress => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        let mut control = world.fetch_mut(control).unwrap();
                        animation.target = 0.5;
                        animation.last_update = Instant::now();
                        control.refreshing = true;
                    }
                    Interact::ButtonRelease => {
                        let mut animation = world.fetch_mut(animation).unwrap();
                        let mut control = world.fetch_mut(control).unwrap();
                        animation.target = 1.0;
                        animation.last_update = Instant::now();
                        control.refreshing = true;
                    }
                    Interact::WidgetEnabled => todo!(),
                    Interact::WidgetDisabled => todo!(),
                    Interact::Resized => todo!(),
                },
            );
        });
    }
}
