use crate::elements::Element;

struct Button;

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: [i32; 4],
    action: Box<dyn FnMut()>,
}
impl Element for ButtonRaw {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "button".into()
    }
    fn border(&self) -> [i32; 4] {
        self.rect
    }
}
impl ButtonRaw {
    pub fn new(rect: [i32; 4], action: impl FnMut() + 'static) -> ButtonRaw {
        ButtonRaw {
            rect,
            action: Box::new(action),
        }
    }
    pub fn pressed(&mut self) {
        (self.action)()
    }
}
