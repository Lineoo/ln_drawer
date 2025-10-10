use crate::{
    elements::{OrderElement, OrderElementExt},
    interface::{Interface, Square},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    tools::pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    world::{Element, Modifier, WorldCell, WorldCellEntry},
};

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: Rectangle,
    order: isize,
    square: Square,
    collider: PointerCollider,
    action: Box<dyn FnMut(&WorldCell)>,
}
impl Element for ButtonRaw {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
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

        entry.observe::<Modifier<Position>>(move |modifier, entry| {
            let mut this = entry.fetch_mut::<ButtonRaw>(entry.handle()).unwrap();
            this.rect.origin = modifier.invoke(this.rect.origin);
        });

        self.register_order(entry.handle(), entry.world());

        entry.register::<PointerCollider>(|this| {
            &this.downcast_ref::<ButtonRaw>().unwrap().collider
        });
    }
}
impl OrderElement for ButtonRaw {
    fn get_order(&self) -> isize {
        self.order
    }

    fn set_order(&mut self, order: isize) {
        self.order = order;
        self.square.set_z_order(order);
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
            z_order: 0,
        };
        ButtonRaw {
            rect,
            order: 0,
            square,
            collider,
            action: Box::new(action),
        }
    }
}
