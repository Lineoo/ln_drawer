pub enum WidgetHover {
    HoverEnter,
    HoverLeave,
}

pub enum WidgetButton {
    ButtonPress,
    ButtonRelease,
}

pub struct WidgetModified;

pub struct WidgetSelect(pub Option<i32>);

pub struct WidgetClick;

pub struct WidgetSwitch;

pub struct WidgetDestroyed;
