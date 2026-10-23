use ln_world::Element;

pub struct Val<T> {
    pub val: T,
}

/// maybe change, maybe not
pub struct ValChanged;

impl<T: Send + 'static> Element for Val<T> {}

impl<T> Val<T> {
    pub const fn new(val: T) -> Self {
        Val { val }
    }
}
