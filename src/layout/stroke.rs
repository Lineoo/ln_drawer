use crate::{elements::StrokeLayer, interface::Interface};

/// The main component for selection.
pub struct StrokeManager {
    cursor: [i32; 2],
    cursor_down: bool,
    curr_color: [u8; 4],
    layers: Vec<StrokeLayer>,
}
impl StrokeManager {
    pub fn new() -> StrokeManager {
        StrokeManager {
            cursor: [0, 0],
            cursor_down: false,
            curr_color: [0xff; 4],
            layers: vec![StrokeLayer::new()],
        }
    }

    pub fn cursor_position(&mut self, point: [i32; 2], interface: &mut Interface) {
        self.cursor = point;
        if self.cursor_down {   
            self.layers[0].write_pixel(self.cursor, self.curr_color, interface);
        }
    }

    pub fn cursor_pressed(&mut self, color: [u8; 4], interface: &mut Interface) {
        self.cursor_down = true;
        self.layers[0].write_pixel(self.cursor, color, interface);
    }

    pub fn cursor_released(&mut self) {
        self.cursor_down = false;
    }
}
