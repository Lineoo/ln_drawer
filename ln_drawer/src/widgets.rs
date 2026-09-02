use crate::measures::Rectangle;

pub mod button;
pub mod echo;
pub mod palette;
pub mod panel;
pub mod renderer;
pub mod shaders;
pub mod slider;
pub mod tabs;
pub mod container;

pub enum WidgetHover {
    Enter,
    Leave,
}

pub struct WidgetRectangle(pub Rectangle);
pub struct WidgetVisible(pub bool);

pub struct SetWidgetRectangle(pub Rectangle);
pub struct SetWidgetVisible(pub bool);
