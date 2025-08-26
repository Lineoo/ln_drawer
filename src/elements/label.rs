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

    fn get_border(&self) -> [i32; 4] {
        self.inner.get_border()
    }

    fn get_position(&self) -> [i32; 2] {
        self.inner.get_position()
    }
    
    fn set_position(&mut self, position: [i32; 2]) {
        self.inner.set_position(position);
    }
    
}
impl Label {
    pub fn new(rect: [i32; 4], text: String, interface: &mut Interface) -> Label {
        let inner = interface.create_text(rect, &text);
        Label { text, inner }
    }
}
