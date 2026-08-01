use crate::measures::Rectangle;

pub mod button;
pub mod palette;
pub mod panel;
pub mod renderer;
pub mod slider;
pub mod tabs;

pub enum WidgetHover {
    Enter,
    Leave,
}

pub struct ButtonClick;
pub enum ButtonAction {
    Press,
    Release,
}

pub struct WidgetRectangle(pub Rectangle);
pub struct WidgetVisible(pub bool);
pub struct WidgetDestroyed;

pub struct SetWidgetRectangle(pub Rectangle);
pub struct SetWidgetVisible(pub bool);
pub struct SetWidgetAnimatedRectangle(pub Rectangle);
