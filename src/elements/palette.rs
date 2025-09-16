use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    elements::{
        Element, ElementExt, PositionChanged, PositionedElement,
        intersect::{IntersectHit, Intersection},
    },
    interface::{Interface, Painter},
    lnwin::PointerEvent,
    world::{ElementHandle, World, WorldCell},
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

        let intersect = world.insert(Intersection {
            host: handle,
            rect: self.painter.get_rect(),
            z_order: 100,
        });
        world.entry(intersect).unwrap().depend(handle);

        this.observe::<IntersectHit>(move |event, world| match event.0 {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = world.fetch_mut::<Palette>(handle).unwrap();
                this.set_knob_position(point);
            }
            _ => (),
        });

        this.observe::<PositionChanged>(move |_event, world| {
            let position = world
                .fetch::<dyn PositionedElement>(handle)
                .unwrap()
                .get_position();
            let mut intersect = world.fetch_mut::<Intersection>(intersect).unwrap();
            
            intersect.rect[2] = intersect.rect[2] - intersect.rect[0] + position[0];
            intersect.rect[3] = intersect.rect[3] - intersect.rect[1] + position[1];
            intersect.rect[0] = position[0];
            intersect.rect[1] = position[1];
        });

        self.register::<dyn PositionedElement>(handle, world);
    }
}
impl PositionedElement for Palette {
    fn get_position(&self) -> [i32; 2] {
        self.painter.get_position()
    }

    fn set_position(&mut self, position: [i32; 2]) {
        self.painter.set_position(position);
    }
}
impl Palette {
    pub fn new(position: [i32; 2], world: &mut World) -> Palette {
        let interface = world.single_mut::<Interface>().unwrap();

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
            [
                position[0],
                position[1],
                position[0] + 128,
                position[1] + 128,
            ],
            data,
        );

        // Picker Knob //
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
        let mut knob = interface.create_painter_with(rect, data);
        knob.set_z_order(1);

        Palette { painter, knob }
    }

    pub fn get_knob_position(&self) -> [i32; 2] {
        let raw_pos = self.knob.get_position();
        [raw_pos[0] + 1, raw_pos[1] + 1]
    }

    pub fn set_knob_position(&mut self, position: [i32; 2]) {
        let rect = self.painter.get_rect();
        let x = position[0].clamp(rect[0], rect[2] - 1);
        let y = position[1].clamp(rect[1], rect[3] - 1);

        self.knob.set_position([x - 1, y - 1]);
    }

    pub fn pick_color(&self) -> [u8; 4] {
        let base_position = self.painter.get_position();
        let knob_position = self.get_knob_position();

        let x = knob_position[0] - base_position[0];
        let y = knob_position[1] - base_position[1];
        let cx = x.rem_euclid(WIDTH as i32);
        let cy = y.rem_euclid(HEIGHT as i32);

        let hsl: Hsl = Hsl::new(0.5, cx as f32 / WIDTH as f32, cy as f32 / HEIGHT as f32);
        let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

        [rgb.red, rgb.blue, rgb.green, 255]
    }
}
