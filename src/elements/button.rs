use crate::{
    interface::{Interface, StandardSquare},
    lnwin::PointerEvent,
    measures::{Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    world::{Element, InsertElement, WorldCell, WorldCellEntry},
};

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    square: StandardSquare,
    action: Box<dyn FnMut(&WorldCell)>,
}
impl Element for ButtonRaw {}
impl InsertElement for ButtonRaw {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<PointerHit>(move |event, entry| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = entry.fetch_mut().unwrap();
                (this.action)(entry.world());
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

        entry.getter::<PointerCollider>(|this| PointerCollider {
            rect: this.square.get_rect(),
            z_order: this.square.get_z_order(),
        });

        entry.getter::<Rectangle>(|this| this.square.get_rect());
        entry.setter::<Rectangle>(|this, rect| this.square.set_rect(rect));
    }
}
impl ButtonRaw {
    pub fn new(
        rect: Rectangle,
        z_order: ZOrder,
        action: impl FnMut(&WorldCell) + 'static,
        interface: &mut Interface,
    ) -> ButtonRaw {
        ButtonRaw {
            square: StandardSquare::new(rect, z_order, false, interface),
            action: Box::new(action),
        }
    }
}
