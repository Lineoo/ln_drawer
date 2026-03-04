use crate::{measures::Rectangle, world::Handle};

pub mod button;
pub mod check_button;
pub mod menu;
pub mod panel;
pub mod resizable;
pub mod color_picker;

/// Attach a headless widget to a specific element.
pub struct Attach<T, U> {
    pub widget: Handle<T>,
    pub target: Handle<U>,
}

pub enum WidgetHover {
    HoverEnter,
    HoverLeave,
}

pub enum WidgetButton {
    ButtonPress,
    ButtonRelease,
}

pub struct WidgetClick;
pub struct WidgetSelect(pub Option<i32>);

pub struct WidgetRectangle(pub Rectangle);
pub struct WidgetChecked(pub bool);

pub struct WidgetDestroyed;
