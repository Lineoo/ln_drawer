use crate::{
    elements::{
        Element, OrderElement, OrderElementExt, PositionElementExt, PositionedElement,
        tools::pointer::{PointerHit, PointerHitExt, PointerHittable},
    },
    interface::{Interface, Square},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    world::{ElementHandle, WorldCell},
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
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.observe::<PointerHit>(move |event, world| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
                (this.action)(world);
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

        let mut interface = world.single_mut::<Interface>().unwrap();
        let square = interface.create_square(self.rect, [1.0, 1.0, 1.0, 0.6]);
        square.set_z_order(self.order);
        square.set_visible(false);
        self.square = Some(square);

        self.register_position(handle, world);
        self.register_order(handle, world);
        self.register_hittable(handle, world);
    }
}
impl PositionedElement for ButtonRaw {
    fn get_position(&self) -> Position {
        self.rect.origin
    }

    fn set_position(&mut self, position: Position) {
        self.rect.origin = position;
        if let Some(square) = &mut self.square {
            square.set_rect(self.rect);
        }
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
