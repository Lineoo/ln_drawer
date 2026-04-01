use ::palette::Hsla;

use crate::{measures::Rectangle, world::Handle};

pub mod button;
pub mod check_button;
pub mod expandable;
pub mod menu;
pub mod palette;
pub mod panel;
pub mod resizable;
pub mod translatable;

/// Attach a headless widget to a specific element.
#[deprecated]
pub struct Attach<T, U> {
    pub widget: Handle<T>,
    pub target: Handle<U>,
}

/// Send when widget's hovering status is changed.
pub enum WidgetHover {
    HoverEnter,
    HoverLeave,
}

/// Send when widget's status as a button is changed.
pub enum WidgetButton {
    ButtonPress,
    ButtonRelease,
}

/// Send when widget is clicked.
pub struct WidgetClick;

/// Send when widget's selection is changed.
pub struct WidgetSelect(pub Option<i32>);

/// Send when widget's rectangle data is changed.
pub struct WidgetRectangle(pub Rectangle);

/// Send when widget's checked data is changed.
pub struct WidgetChecked(pub bool);

/// Send when widget's color data formatted in hsl is changed.
pub struct WidgetHsla(pub Hsla);

/// Send when widget is folded or expanded.
pub struct WidgetExpanded(pub bool);

/// Send when widget is enabled or disabled.
pub struct WidgetEnabled(pub bool);

/// Send when widget is about to be removed.
pub struct WidgetDestroyed;
