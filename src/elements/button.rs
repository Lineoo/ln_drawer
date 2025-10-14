use crate::{
    interface::{Interface, Square},
    lnwin::PointerEvent,
    measures::{Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    world::{Element, InsertElement, WorldCell, WorldCellEntry},
};

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: Rectangle,
    square: Square,
    collider: PointerCollider,
    action: Box<dyn FnMut(&WorldCell)>,
}
impl Element for ButtonRaw {}
impl InsertElement for ButtonRaw {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<PointerHit>(move |event, entry| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = entry.fetch_mut::<ButtonRaw>(entry.handle()).unwrap();
                (this.action)(entry.world());
            }
        });

        entry.observe::<PointerEnter>(move |_event, entry| {
            let this = entry.fetch::<ButtonRaw>(entry.handle()).unwrap();
            this.square.set_visible(true);
        });
        entry.observe::<PointerLeave>(move |_event, entry| {
            let this = entry.fetch::<ButtonRaw>(entry.handle()).unwrap();
            this.square.set_visible(false);
        });

        entry.getter::<PointerCollider>(|this| this.collider);
    }
}
impl ButtonRaw {
    pub fn new(
        rect: Rectangle,
        action: impl FnMut(&WorldCell) + 'static,
        interface: &mut Interface,
    ) -> ButtonRaw {
        let square = interface.create_square(rect, [1.0, 1.0, 1.0, 0.6]);
        square.set_visible(false);
        let collider = PointerCollider {
            rect: square.get_rect(),
            z_order: ZOrder::default(),
        };
        ButtonRaw {
            rect,
            square,
            collider,
            action: Box::new(action),
        }
    }
}
