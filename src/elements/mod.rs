mod image;

use std::any::Any;

pub use image::Image;

pub trait Element: Any {
    fn name(&self) -> std::borrow::Cow<'_, str>;
    fn border(&self) -> [i32; 4];
}