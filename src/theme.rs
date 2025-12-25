use palette::Srgba;

use crate::{
    render::rounded::RoundedRectDescriptor,
    widgets::{button::Button, events::Interact},
    world::{Element, Handle, World},
};

/// Trigger this to *try* to attach a headless widget to a specific theme
pub struct Attach<T>(pub Handle<T>);

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    back_color: Srgba,
    front_color: Srgba,
    press_color: Srgba,
    roundness: f32,
    pad: i32,
}

impl Default for Luni {
    fn default() -> Self {
        Self {
            back_color: Srgba::new(0.1, 0.1, 0.1, 0.9),
            front_color: Srgba::new(0.3, 0.3, 0.3, 1.0),
            press_color: Srgba::new(0.5, 0.5, 0.5, 1.0),
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
                color: this.front_color,
                shrink: this.roundness,
                value: this.roundness,
                visible: false,
            });

            world.dependency(back_frame, button.handle());
            world.dependency(front_frame, button.handle());

            let this = this.handle();
            world.observer(button.handle(), move |interact: &Interact, world, _| {
                let this = world.fetch(this).unwrap();
                let back_frame = world.fetch(back_frame).unwrap();
                let mut front_frame = world.fetch_mut(front_frame).unwrap();

                match interact {
                    Interact::HoverEnter => {
                        front_frame.visible = true;
                    }
                    Interact::HoverLeave => {
                        front_frame.visible = false;
                    }
                    Interact::ButtonPress => {
                        front_frame.rect = back_frame.rect;
                        front_frame.color = this.press_color;
                        front_frame.shrink = back_frame.shrink;
                        front_frame.value = back_frame.value;
                    }
                    Interact::ButtonRelease => {
                        front_frame.rect = back_frame.rect.expand(-this.pad);
                        front_frame.color = this.front_color;
                        front_frame.shrink = back_frame.shrink;
                        front_frame.value = back_frame.value;
                    }
                    Interact::WidgetEnabled => todo!(),
                    Interact::WidgetDisabled => todo!(),
                    Interact::Resized => todo!(),
                }
            });
        });
    }
}
