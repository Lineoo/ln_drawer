use crate::measures::Rectangle;

pub enum Interact {
    HoverEnter,
    HoverLeave,
    ButtonPress,
    ButtonRelease,
    PropertyChange,
}

pub enum InteractSelect {
    HoverItem(Rectangle)
}

pub struct Click;

pub struct Switch;
