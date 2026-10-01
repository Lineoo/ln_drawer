use ln_world::Handle;

use crate::render::camera::Camera;

pub struct WidgetCommon {
    pub camera: Handle<Camera>,
    pub visible: bool,
}
