use crate::elements::Element;

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    rect: [i32; 4],
    action: Box<dyn FnMut()>,
}
impl Element for ButtonRaw {}
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

    pub fn get_position(&self) -> [i32; 2] {
        [self.rect[0], self.rect[1]]
    }

    pub fn set_position(&mut self, position: [i32; 2]) {
        let (width, height) = (self.width(), self.height());
        self.rect[0] = position[0];
        self.rect[1] = position[1];
        self.rect[2] = position[0] + width as i32;
        self.rect[3] = position[1] + height as i32;
    }

    fn width(&self) -> u32 {
        (self.rect[0] - self.rect[2]).unsigned_abs()
    }

    fn height(&self) -> u32 {
        (self.rect[1] - self.rect[3]).unsigned_abs()
    }
}
