mod button;
mod image;
pub mod intersect;
mod menu;
mod palette;
mod player;
mod stroke;
mod text;

pub use button::ButtonRaw;
pub use image::Image;
pub use intersect::Intersect;
pub use menu::Menu;
pub use palette::Palette;
pub use stroke::StrokeLayer;
pub use text::{Text, TextManager};

use crate::{
    measures::Position,
    world::{Element, ElementHandle, WorldCell},
};

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
