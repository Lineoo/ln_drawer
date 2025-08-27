use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    elements::Element,
    interface::{Interface, Painter},
    layout::world::{ElementHandle, World},
};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

pub struct Palette {
    painter: Painter,
}
impl Element for Palette {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "button".into()
    }

    fn get_border(&self) -> [i32; 4] {
        self.painter.get_rect()
    }

    fn get_position(&self) -> [i32; 2] {
        self.painter.get_position()
    }

    fn set_position(&mut self, position: [i32; 2]) {
        self.painter.set_position(position);
    }

    fn z_index(&self) -> i64 {
        10
    }
}
impl Palette {
    pub fn new(position: [i32; 2], interface: &mut Interface) -> Palette {
        let mut data = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        for x in 0..128 {
            for y in 0..128 {
                let start = (x + y * 128) * 4;
                let hsl: Hsl = Hsl::new(0.5, x as f32 / 128.0, (127 - y) as f32 / 128.0);
                let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();
                data[start] = rgb.red;
                data[start + 1] = rgb.blue;
                data[start + 2] = rgb.green;
                data[start + 3] = 255;
            }
        }

        Palette {
            painter: interface.create_painter_with(
                [
                    position[0],
                    position[1],
                    position[0] + 128,
                    position[1] + 128,
                ],
                data,
            ),
        }
    }
}

pub struct PaletteKnob {
    position: [i32; 2],
    painter: Painter,
    palette: ElementHandle,
}
impl Element for PaletteKnob {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "palette_knob".into()
    }

    fn get_border(&self) -> [i32; 4] {
        let border = self.painter.get_rect();
        [border[0] - 1, border[1] - 1, border[2] + 1, border[3] + 1]
    }

    fn get_position(&self) -> [i32; 2] {
        self.position
    }

    fn set_position(&mut self, position: [i32; 2]) {
        self.position = position;
        self.painter.set_rect([
            self.position[0] - 1,
            self.position[1] - 1,
            self.position[0] + 2,
            self.position[1] + 2,
        ]);
    }

    fn z_index(&self) -> i64 {
        100
    }
}
impl PaletteKnob {
    pub fn new(palette: ElementHandle, interface: &mut Interface) -> PaletteKnob {
        let position = [0, 0];
        let rect = [
            position[0] - 1,
            position[1] - 1,
            position[0] + 2,
            position[1] + 2,
        ];

        let mut data = vec![0u8; 3 * 3 * 4];
        for x in 0..3 {
            for y in 0..3 {
                if x == 0 || y == 0 || x == 2 || y == 2 {
                    let start = (x + y * 3) * 4;
                    data[start] = 0xff;
                    data[start + 1] = 0xff;
                    data[start + 2] = 0xff;
                    data[start + 3] = 0xff;
                }
            }
        }
        let painter = interface.create_painter_with(rect, data);
        painter.set_z_order(1);

        PaletteKnob {
            position: [0, 0],
            painter,
            palette,
        }
    }

    pub fn get_color(&self, world: &World) -> [u8; 4] {
        if let Some(palette) = world.fetch::<Palette>(self.palette) {
            let x = self.position[0] - palette.get_position()[0];
            let y = self.position[1] - palette.get_position()[1];
            let cx = x.rem_euclid(WIDTH as i32);
            let cy = y.rem_euclid(HEIGHT as i32);

            let hsl: Hsl = Hsl::new(0.5, cx as f32 / WIDTH as f32, cy as f32 / HEIGHT as f32);
            let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

            [rgb.red, rgb.blue, rgb.green, 255]
        } else {
            [0xff, 0xff, 0xff, 255]
        }
    }
}
