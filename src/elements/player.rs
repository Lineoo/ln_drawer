use winit::{
    event::ElementState,
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    elements::{Image, OrderElement},
    interface::Interface,
    lnwin::PointerEvent,
    measures::Delta,
    tools::{
        focus::{Focus, FocusInput, FocusOn},
        pointer::{PointerCollider, PointerHit},
    },
    world::{Element, WorldCellEntry},
};

pub struct Player {
    image: Image,
    collider: PointerCollider,
}
impl Element for Player {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe(move |PointerHit(pointer), entry| {
            if let PointerEvent::Pressed(_) = pointer {
                let mut focus = entry.single_mut::<Focus>().unwrap();
                focus.set(Some(entry.handle()), &entry);
            }
        });
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
    }
}
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
