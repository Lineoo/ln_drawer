mod button;
mod image;
pub mod intersect;
mod menu;
mod palette;
mod stroke;
mod text;
mod player;

use std::any::Any;

pub use button::ButtonRaw;
pub use image::Image;
pub use intersect::Intersect;
pub use menu::Menu;
pub use palette::Palette;
pub use stroke::StrokeLayer;
pub use text::{Text, TextManager};

use crate::{
    measures::Position,
    world::{ElementHandle, WorldCell},
};

#[expect(unused_variables)]
pub trait Element: Any {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {}
    fn when_removed(&mut self, handle: ElementHandle, world: &WorldCell) {}
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
    fn get_position(&self) -> Position;
    fn set_position(&mut self, position: Position);
}

trait PositionElementExt: PositionedElement + Sized {
    fn register_position(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.register::<dyn PositionedElement>(|this| this.downcast_ref::<Self>().unwrap());
        this.register_mut::<dyn PositionedElement>(|this| this.downcast_mut::<Self>().unwrap());
    }
}
impl<T: PositionedElement> PositionElementExt for T {}

pub trait OrderElement: Element {
    fn get_order(&self) -> isize;
    fn set_order(&mut self, order: isize);
}

trait OrderElementExt: OrderElement + Sized {
    fn register_order(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        this.register::<dyn OrderElement>(|this| this.downcast_ref::<Self>().unwrap());
        this.register_mut::<dyn OrderElement>(|this| this.downcast_mut::<Self>().unwrap());
    }
}
impl<T: OrderElement> OrderElementExt for T {}
