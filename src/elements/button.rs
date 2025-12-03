use crate::{
    interface::{Interface, StandardSquare},
    lnwin::PointerEvent,
    measures::{Delta, Rectangle, ZOrder},
    text::Text,
    tools::{
        pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    },
    world::{Element, WorldCellEntry},
};

type ButtonAction = Box<dyn FnMut(WorldCellEntry<ButtonRaw>)>;

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    square: StandardSquare,
    action: ButtonAction,
}
impl Element for ButtonRaw {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<PointerHit>(move |event, entry| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = entry.fetch_mut().unwrap();
                let mut temp: ButtonAction = Box::new(|_| ());
                std::mem::swap(&mut this.action, &mut temp);
                drop(this);

                temp(entry.clone());

                let mut this = entry.fetch_mut().unwrap();
                std::mem::swap(&mut this.action, &mut temp);
            }
        });

        entry.observe::<PointerEnter>(move |_event, entry| {
            let mut this = entry.fetch_mut().unwrap();
            this.square.set_visible(true);
        });
        entry.observe::<PointerLeave>(move |_event, entry| {
            let mut this = entry.fetch_mut().unwrap();
            this.square.set_visible(false);
        });
    }
}
impl ButtonRaw {
    pub fn new(
        rect: Rectangle,
        z_order: ZOrder,
        action: impl FnMut(WorldCellEntry<Self>) + 'static,
        interface: &mut Interface,
    ) -> ButtonRaw {
        ButtonRaw {
            square: StandardSquare::new(
                rect,
                z_order,
                false,
                palette::Srgba::new(1.0, 1.0, 1.0, 1.0),
                interface,
            ),
            action: Box::new(action),
        }
    }
}
