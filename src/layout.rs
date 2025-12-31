use crate::measures::Rectangle;

pub mod resizable;
pub mod transform;
pub mod translatable;
pub mod animator;

pub enum Layout {
    Rectangle(Rectangle),
    Alpha(f32),
}
