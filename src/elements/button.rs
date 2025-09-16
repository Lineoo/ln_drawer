use crate::{
    elements::{
        Element, ElementExt, PositionChanged, PositionedElement,
        intersect::{IntersectHit, Intersection},
    },
    interface::{Interface, Square},
    lnwin::PointerEvent,
    world::{ElementHandle, WorldCell},
};

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: [i32; 4],
    action: Box<dyn FnMut()>,
    square: Option<Square>,
}
impl Element for ButtonRaw {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        let intersect = world.insert(Intersection {
            host: handle,
            rect: self.rect,
            z_order: 0,
        });
        world.entry(intersect).unwrap().depend(handle);

        this.observe::<IntersectHit>(move |event, world| {
            if let IntersectHit(PointerEvent::Pressed(_)) = event {
                let mut this = world.fetch_mut::<ButtonRaw>(handle).unwrap();
                (this.action)();
            }
        });
        this.observe::<PositionChanged>(move |_event, world| {
            let position = world
                .fetch::<dyn PositionedElement>(handle)
                .unwrap()
                .get_position();
            let mut intersect = world.fetch_mut::<Intersection>(intersect).unwrap();
            
            intersect.rect[2] = intersect.rect[2] - intersect.rect[0] + position[0];
            intersect.rect[3] = intersect.rect[3] - intersect.rect[1] + position[1];
            intersect.rect[0] = position[0];
            intersect.rect[1] = position[1];
        });

        let mut interface = world.single_mut::<Interface>().unwrap();
        self.square = Some(interface.create_square(self.rect, [1.0, 1.0, 1.0, 0.6]));

        self.register::<dyn PositionedElement>(handle, world);
    }
}
impl PositionedElement for ButtonRaw {
    fn get_position(&self) -> [i32; 2] {
        [self.rect[0], self.rect[1]]
    }

    fn set_position(&mut self, position: [i32; 2]) {
        let (width, height) = (self.width(), self.height());
        self.rect[0] = position[0];
        self.rect[1] = position[1];
        self.rect[2] = position[0] + width as i32;
        self.rect[3] = position[1] + height as i32;
    }
}
impl ButtonRaw {
    pub fn new(rect: [i32; 4], action: impl FnMut() + 'static) -> ButtonRaw {
        ButtonRaw {
            rect,
            action: Box::new(action),
            square: None,
        }
    }

    fn width(&self) -> u32 {
        (self.rect[0] - self.rect[2]).unsigned_abs()
    }

    fn height(&self) -> u32 {
        (self.rect[1] - self.rect[3]).unsigned_abs()
    }
}
