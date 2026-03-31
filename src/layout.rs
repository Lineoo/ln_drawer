use hashbrown::HashMap;

use crate::{
    measures::Rectangle,
    world::{Element, Handle, World},
};

pub mod transform;

#[deprecated]
pub struct LayoutRectangle(pub Rectangle);

/// Record registration from widgets of their layout controls.
#[derive(Default)]
pub struct LayoutControls(pub HashMap<Handle, Handle<LayoutControl>>);

/// Register layout calculation of different widgets
#[derive(Default)]
pub struct LayoutControl {
    /// Calculate rectangle. Input desired rectangle, return final rectangle.
    pub rectangle: Option<Box<dyn FnMut(&World, Rectangle) -> Rectangle>>,
}

impl Element for LayoutControls {}
impl Element for LayoutControl {}
