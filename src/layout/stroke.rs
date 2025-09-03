use glam::Vec2;

use crate::{
    elements::{PaletteKnob, StrokeLayer},
    interface::Interface,
    layout::world::World,
};

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
        if self.cursor_down {
            let mut vernier = Vec2::new(self.cursor[0] as f32, self.cursor[1] as f32);
            let destination = Vec2::new(point[0] as f32, point[1] as f32);
            while vernier != destination {
                self.layers[0].write_pixel(
                    [vernier.x.floor() as i32, vernier.y.floor() as i32],
                    self.curr_color,
                    interface,
                );
                vernier = vernier.move_towards(destination, 0.7);
            }
        }
        self.cursor = point;
    }

    pub fn cursor_pressed(&mut self, color: [u8; 4], interface: &mut Interface) {
        self.cursor_down = true;
        self.layers[0].write_pixel(self.cursor, color, interface);
    }

    pub fn cursor_released(&mut self) {
        self.cursor_down = false;
    }

    pub fn update_color(&mut self, world: &mut World) {
        for knob in world.elements::<PaletteKnob>() {
            self.curr_color = knob.get_color(world);
        }
    }
}
