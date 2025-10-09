use crate::{
    elements::{OrderElement, OrderElementExt},
    interface::{Interface, Square},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    tools::pointer::{PointerHit, PointerHitExt, PointerHittable},
    world::{Element, Modifier, WorldCell, WorldCellEntry},
};

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: Rectangle,
    order: isize,
    action: Box<dyn FnMut(&WorldCell)>,
    square: Option<Square>,
}
impl Element for ButtonRaw {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe::<PointerHit>(move |event, entry| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = entry.fetch_mut::<ButtonRaw>(entry.handle()).unwrap();
                (this.action)(entry.world());
            }
        });

        // this.observe::<PointerEnter>(move |_event, world| {
        //     let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
        //     this.square.as_mut().unwrap().set_visible(true);
        // });
        // this.observe::<PointerLeave>(move |_event, world| {
        //     let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
        //     this.square.as_mut().unwrap().set_visible(false);
        // });

        entry.observe::<Modifier<Position>>(move |modifier, entry| {
            let mut this = entry.fetch_mut::<ButtonRaw>(entry.handle()).unwrap();
            this.rect.origin = modifier.invoke(this.rect.origin);
        });

        let mut interface = entry.single_mut::<Interface>().unwrap();
        let square = interface.create_square(self.rect, [1.0, 1.0, 1.0, 0.6]);
        square.set_z_order(self.order);
        square.set_visible(false);
        self.square = Some(square);

        self.register_order(entry.handle(), entry.world());
        self.register_hittable(entry.handle(), entry.world());
    }
}
impl OrderElement for ButtonRaw {
    fn get_order(&self) -> isize {
        self.order
    }

    fn set_order(&mut self, order: isize) {
        self.order = order;
        if let Some(square) = &mut self.square {
            square.set_z_order(order);
        }
    }
}
impl PointerHittable for ButtonRaw {
    fn get_hitting_rect(&self) -> Rectangle {
        self.rect
    }

    fn get_hitting_order(&self) -> isize {
        self.order
    }
}
impl ButtonRaw {
    pub fn new(rect: Rectangle, action: impl FnMut(&WorldCell) + 'static) -> ButtonRaw {
        ButtonRaw {
            rect,
            order: 0,
            action: Box::new(action),
            square: None,
        }
    }
}
