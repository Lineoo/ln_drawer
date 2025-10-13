use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    interface::{Interface, Painter, Wireframe},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, WorldCellEntry},
};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

pub struct Palette {
    main: Painter,
    main_knob: Wireframe,
}
impl Element for Palette {
    fn when_inserted(&mut self, mut entry: WorldCellEntry) {
        entry.observe::<PointerHit>(move |event, entry| match event.0 {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = entry.fetch_mut::<Palette>(entry.handle()).unwrap();
                this.main_knob.set_rect(Rectangle {
                    origin: point,
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
            let relative = orig - knob_orig;

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
        let main = interface.create_painter_with(
            Rectangle {
                origin: position,
                extend: Delta::splat(128),
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

        Palette { main, main_knob }
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
