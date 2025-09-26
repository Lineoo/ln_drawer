use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    elements::{
        Element, PositionElementExt, PositionedElement,
        tools::pointer::{PointerHit, PointerHitExt, PointerHittable},
    },
    interface::{Interface, Painter},
    lnwin::PointerEvent,
    measures::{Position, Rectangle},
    world::{ElementHandle, WorldCell},
};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

pub struct Palette {
    painter: Painter,
    knob: Painter,
}
impl Element for Palette {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();

        this.observe::<PointerHit>(move |event, world| match event.0 {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = world.fetch_mut::<Palette>(handle).unwrap();
                this.set_knob_position(point);
            }
            _ => (),
        });

        self.register_position(handle, world);
        self.register_hittable(handle, world);
    }
}
impl PositionedElement for Palette {
    fn get_position(&self) -> Position {
        Position::from_array(self.painter.get_position())
    }

    fn set_position(&mut self, position: Position) {
        let origin = self.get_position();
        let delta = position - origin;

        let knob_origin = self.get_knob_position();
        let knob_position = knob_origin + delta;

        self.painter.set_position(position.into_array());
        self.knob
            .set_position([knob_position.x - 1, knob_position.y - 1]);
    }
}
impl PointerHittable for Palette {
    fn get_hitting_rect(&self) -> Rectangle {
        Rectangle::from_array(self.painter.get_rect())
    }

    fn get_hitting_order(&self) -> isize {
        self.painter.get_z_order()
    }
}
impl Palette {
    pub fn new(position: Position, interface: &mut Interface) -> Palette {
        // Palette //
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
        let painter = interface.create_painter_with(
            [position.x, position.y, position.x + 128, position.y + 128],
            data,
        );

        // Picker Knob //
        let rect = [
            position.x - 1,
            position.y - 1,
            position.x + 2,
            position.y + 2,
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
        let mut knob = interface.create_painter_with(rect, data);
        knob.set_z_order(1);

        Palette { painter, knob }
    }

    pub fn get_knob_position(&self) -> Position {
        let raw_pos = self.knob.get_position();
        Position::from_array([raw_pos[0] + 1, raw_pos[1] + 1])
    }

    pub fn set_knob_position(&mut self, position: Position) {
        let rect = self.painter.get_rect();
        let x = position.x.clamp(rect[0], rect[2] - 1);
        let y = position.y.clamp(rect[1], rect[3] - 1);

        self.knob.set_position([x - 1, y - 1]);
    }

    pub fn pick_color(&self) -> [u8; 4] {
        let base_position = self.get_position();
        let knob_position = self.get_knob_position();

        let x = knob_position.x - base_position.x;
        let y = knob_position.y - base_position.y;
        let cx = x.rem_euclid(WIDTH as i32);
        let cy = y.rem_euclid(HEIGHT as i32);

        let hsl: Hsl = Hsl::new(0.5, cx as f32 / WIDTH as f32, cy as f32 / HEIGHT as f32);
        let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

        [rgb.red, rgb.blue, rgb.green, 255]
    }
}
