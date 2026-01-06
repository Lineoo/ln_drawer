pub enum Interact {
    HoverEnter,
    HoverLeave,
    ButtonPress,
    ButtonRelease,
    PropertyChange,
}

pub enum InteractSelect {
    Entry(Option<i32>),
}

pub struct Click;

pub struct Switch;
