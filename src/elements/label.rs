use crate::{
    elements::Element,
    interface::{Interface, Text},
};

pub struct Label {
    text: String,
    inner: Text,
}
impl Element for Label {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "label".into()
    }
    fn border(&self) -> [i32; 4] {
        self.inner.get_border()
    }
}
impl Label {
    pub fn new(rect: [i32; 4], text: String, interface: &mut Interface) -> Label {
        let inner = interface.create_text(rect, &text);
        Label { text, inner }
    }
}
