use winit::{
    event::ElementState,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    elements::Image,
    interface::Interface,
    measures::{Delta, Rectangle},
    tools::{
        focus::{FocusInput, FocusOn, Focusable, FocusableExt},
        pointer::{PointerHitExt, PointerHittable},
    },
    world::{Element, WorldCellEntry},
};

pub struct Player {
    image: Image,
}
impl Element for Player {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe(move |FocusOn, _entry| {
            println!("player is here!");
        });
        entry.observe(move |FocusInput(event), entry| {
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

            let mut this = entry.fetch_mut_raw::<Player>(entry.handle()).unwrap();
            
            let position = this.image.get_position();
            this.image.set_position(position + delta);
        });

        self.register_hittable(entry.handle(), entry.world());
        self.register_focus(entry.handle(), entry.world());
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
