mod image;

use std::any::Any;

pub use image::Image;

pub trait Element: Any {
    fn name(&self) -> std::borrow::Cow<'_, str>;
    fn border(&self) -> [i32; 4];
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
