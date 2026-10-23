use ln_world::Handle;

use crate::{layout::val::Val, render::camera::Camera};

#[derive(Debug, Clone, Copy)]
pub struct WidgetCommon {
    pub camera: Handle<Camera>,
    pub visible: Handle<Val<bool>>,
}
