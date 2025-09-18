use crate::{
    elements::{
        intersect::{IntersectHit, Intersection, PointerEnter, PointerLeave}, Element, OrderElement, OrderElementExt, PositionChanged, PositionElementExt, PositionedElement
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
        let intersect = world.insert(Intersection {
            host: handle,
            rect: self.rect,
            z_order: self.order,
        });
        world.entry(intersect).unwrap().depend(handle);

        this.observe::<IntersectHit>(move |event, world| {
            if let IntersectHit(PointerEvent::Pressed(_)) = event {
                let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
                (this.action)(world);
            }
        });
        this.observe::<PositionChanged>(move |_event, world| {
            let position = world
                .fetch::<dyn PositionedElement>(handle)
                .unwrap()
                .get_position();
            let mut intersect = world.fetch_mut::<Intersection>(intersect).unwrap();

            intersect.rect.origin = position;
        });

        this.observe::<PointerEnter>(move |_event, world| {
            let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
            this.square.as_mut().unwrap().set_visible(true);
        });
        this.observe::<PointerLeave>(move |_event, world| {
            let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
            this.square.as_mut().unwrap().set_visible(false);
        });

        let mut interface = world.single_mut::<Interface>().unwrap();
        let square = interface.create_square(self.rect.into_array(), [1.0, 1.0, 1.0, 0.6]);
        square.set_z_order(self.order);
        square.set_visible(false);
        self.square = Some(square);

        self.register_position(handle, world);
        self.register_order(handle, world);
    }
}
impl PositionedElement for ButtonRaw {
    fn get_position(&self) -> Position {
        self.rect.origin
    }

    fn set_position(&mut self, position: Position) {
        self.rect.origin = position;
        if let Some(square) = &mut self.square {
            square.set_rect(self.rect.into_array());
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
