use crate::{
    measures::Rectangle,
    world::{Element, World},
};

pub mod transform;

pub struct LayoutRectangleAction(pub Box<dyn FnMut(&World, Rectangle) -> Rectangle>);

pub struct LayoutEnableAction(pub Box<dyn FnMut(&World, bool)>);

impl Element for LayoutRectangleAction {}
impl Element for LayoutEnableAction {}
