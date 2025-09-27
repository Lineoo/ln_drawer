use winit::{event::ElementState, keyboard::{KeyCode, PhysicalKey}};

use crate::{
    elements::{Element, Image, PositionedElement},
    interface::Interface,
    measures::{Delta, Rectangle},
    tools::{
        focus::{FocusInput, FocusOn, Focusable, FocusableExt},
        pointer::{PointerHitExt, PointerHittable},
    },
    world::{ElementHandle, WorldCell},
};

pub struct Player {
    image: Image,
}
impl Element for Player {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.observe(move |FocusOn, _world| {
            println!("player is here!");
        });
        this.observe(move |FocusInput(event), world| {
            if event.state != ElementState::Pressed {
                return;
            }

            let delta = match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyW) => Delta::new(0, 1),
                PhysicalKey::Code(KeyCode::KeyA) => Delta::new(-1, 0),
                PhysicalKey::Code(KeyCode::KeyS) => Delta::new(0, -1),
                PhysicalKey::Code(KeyCode::KeyD) => Delta::new(1, 0),
                _ => Delta::splat(0),
            };

            let mut this = world.fetch_mut_raw::<Player>(handle).unwrap();
            let position = this.image.get_position();
            this.image.set_position(position + delta);
        });

        self.register_hittable(handle, world);
        self.register_focus(handle, world);
    }
}
impl PointerHittable for Player {
    fn get_hitting_rect(&self) -> Rectangle {
        self.image.get_hitting_rect()
    }

    fn get_hitting_order(&self) -> isize {
        self.image.get_hitting_order()
    }
}
impl Focusable for Player {}
impl Player {
    pub fn new(interface: &mut Interface) -> Player {
        Player {
            image: Image::from_bytes(include_bytes!("../../res/player.png"), interface).unwrap(),
        }
    }
}
