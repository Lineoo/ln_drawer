use winit::{
    event::ElementState,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    elements::{Image, OrderElement},
    interface::Interface,
    measures::Delta,
    tools::{
        focus::{FocusInput, FocusOn, Focusable, FocusableExt},
        pointer::PointerCollider,
    },
    world::{Element, WorldCellEntry},
};

pub struct Player {
    image: Image,
    collider: PointerCollider,
}
impl Element for Player {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        // TODO focus
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

        entry.register::<PointerCollider>(|this| &this.downcast_ref::<Player>().unwrap().collider);
        self.register_focus(entry.handle(), entry.world());
    }
}
impl Focusable for Player {}
impl Player {
    pub fn new(interface: &mut Interface) -> Player {
        let image = Image::from_bytes(include_bytes!("../../res/player.png"), interface).unwrap();
        let collider = PointerCollider {
            rect: image.get_rect(),
            z_order: image.get_order(),
        };
        Player { image, collider }
    }
}
