use ln_world::{Element, World};

use crate::measures::Rectangle;

pub mod transform;

pub struct LayoutRectangleAction(pub Box<dyn FnMut(&World, Rectangle) -> Rectangle>);

pub struct LayoutEnableAction(pub Box<dyn FnMut(&World, bool)>);

impl Element for LayoutRectangleAction {}
impl Element for LayoutEnableAction {}
