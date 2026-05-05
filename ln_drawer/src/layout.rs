use ln_world::{Element, World};

use crate::measures::Rectangle;

pub mod transform;

#[deprecated]
pub struct LayoutRectangleAction(pub Box<dyn FnMut(&World, Rectangle) -> Rectangle>);

#[deprecated]
pub struct LayoutEnableAction(pub Box<dyn FnMut(&World, bool)>);

impl Element for LayoutRectangleAction {}
impl Element for LayoutEnableAction {}
