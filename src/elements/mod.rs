mod image;

pub use image::Image;

pub trait Element {
    fn name(&self) -> std::borrow::Cow<'_, str>;
    fn border(&self) -> [i32; 4];
}
