mod button;
mod image;
mod intersect;
mod label;
mod palette;
mod stroke;

use std::any::Any;

pub use button::ButtonRaw;
pub use image::Image;
pub use label::Label;
pub use palette::Palette;
pub use stroke::StrokeLayer;

use crate::world::{ElementHandle, WorldQueue};

#[expect(unused_variables)]
pub trait Element: Any {
    fn when_inserted(&mut self, handle: ElementHandle, queue: &mut WorldQueue) {}
    fn when_removed(&mut self, handle: ElementHandle, queue: &mut WorldQueue) {}
}
impl dyn Element {
    pub fn is<T: Any>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}

pub trait PositionedElement: Element {
    fn get_position(&self) -> [i32; 2];
    fn set_position(&mut self, position: [i32; 2]);
}

// TODO ElementBuilder
