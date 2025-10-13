use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    interface::{Interface, Painter, Wireframe},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, WorldCellEntry},
};

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const HUE_HEIGHT: usize = 16;

pub struct Palette {
    main: Painter,
    main_knob: Wireframe,
    hue: Painter,
    hue_knob: Wireframe,
}
impl Element for Palette {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe::<PointerHit>(move |event, entry| match event.0 {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = entry.fetch_mut::<Palette>(entry.handle()).unwrap();
                let rect = this.main.get_rect();
                this.main_knob.set_rect(Rectangle {
                    origin: point.clamp(Rectangle {
                        origin: rect.origin,
                        extend: rect.extend - Delta::splat(1),
                    }),
                    extend: Delta::splat(1),
                });
            }
            _ => (),
        });

        entry.getter::<PointerCollider>(|this| {
            let this = this.downcast_ref::<Palette>().unwrap();
            PointerCollider {
                rect: this.main.get_rect(),
                z_order: this.main.get_z_order(),
            }
        });

        entry.getter::<Rectangle>(|this| {
            let this = this.downcast_ref::<Palette>().unwrap();
            this.main.get_rect()
        });

        entry.setter::<Rectangle>(|this, rect| {
            let this = this.downcast_mut::<Palette>().unwrap();

            let orig = this.main.get_rect().origin;
            let knob_orig = this.main_knob.get_rect().origin;
            let relative = knob_orig - orig;

            this.main.set_rect(rect);
            this.main_knob.set_rect(Rectangle {
                origin: rect.origin + relative,
                extend: Delta::splat(1),
            });
        });
    }
}
impl Palette {
    pub fn new(position: Position, interface: &mut Interface) -> Palette {
        let mut data = vec![0u8; WIDTH * HEIGHT * 4];
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                let start = (x + y * WIDTH) * 4;
                let hsl: Hsl = Hsl::new(
                    0.5,
                    x as f32 / WIDTH as f32,
                    (127 - y) as f32 / HEIGHT as f32,
                );
                let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();
                data[start] = rgb.red;
                data[start + 1] = rgb.blue;
                data[start + 2] = rgb.green;
                data[start + 3] = 255;
            }
        }
        let main = interface.create_painter_with(
            Rectangle {
                origin: position,
                extend: Delta::new(WIDTH as i32, HEIGHT as i32),
            },
            data,
        );

        let main_knob = interface.create_wireframe(
            Rectangle {
                origin: position,
                extend: Delta::splat(1),
            },
            [1.0, 1.0, 1.0, 1.0],
        );
        main_knob.set_z_order(ZOrder::new(1));

        let mut data = vec![0u8; WIDTH * HUE_HEIGHT * 4];
        for x in 0..WIDTH {
            let hsl: Hsl = Hsl::new(x as f32 / WIDTH as f32 * 360.0, 0.8, 0.6);
            let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();
            for y in 0..HUE_HEIGHT {
                let start = (x + y * WIDTH) * 4;
                data[start] = rgb.red;
                data[start + 1] = rgb.blue;
                data[start + 2] = rgb.green;
                data[start + 3] = 255;
            }
        }
        let hue = interface.create_painter_with(
            Rectangle {
                origin: position - Delta::new(0, HUE_HEIGHT as i32),
                extend: Delta::new(WIDTH as i32, HUE_HEIGHT as i32),
            },
            data,
        );

        let hue_knob = interface.create_wireframe(
            Rectangle {
                origin: position - Delta::new(0, HUE_HEIGHT as i32),
                extend: Delta::new(1, HUE_HEIGHT as i32),
            },
            [1.0, 1.0, 1.0, 1.0],
        );
        hue_knob.set_z_order(ZOrder::new(1));

        Palette {
            main,
            main_knob,
            hue,
            hue_knob,
        }
    }

    pub fn pick_color(&self) -> [u8; 4] {
        let base_position = self.main.get_rect().origin;
        let knob_position = self.main_knob.get_rect().origin;

        let x = knob_position.x - base_position.x;
        let y = knob_position.y - base_position.y;
        let cx = x.rem_euclid(WIDTH as i32);
        let cy = y.rem_euclid(HEIGHT as i32);

        let hsl: Hsl = Hsl::new(0.5, cx as f32 / WIDTH as f32, cy as f32 / HEIGHT as f32);
        let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

        [rgb.red, rgb.blue, rgb.green, 255]
    }
}
