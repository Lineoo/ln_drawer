use palette::{FromColor, Hsl, rgb::Rgb};

use crate::{
    interface::{Interface, Painter, Wireframe},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, InsertElement, WorldCellEntry},
};

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const HUE_HEIGHT: usize = 16;

pub struct Palette {
    hue: f32,
    main: Painter,
    main_knob: Wireframe,
}
impl Element for Palette {}
impl InsertElement for Palette {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
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

        entry.getter::<PointerCollider>(|this| PointerCollider {
            rect: this.main.get_rect(),
            z_order: this.main.get_z_order(),
        });

        entry.getter::<Rectangle>(|this| this.main.get_rect());

        entry.setter::<Rectangle>(|this, rect| {
            let orig = this.main.get_rect().origin;
            let knob_orig = this.main_knob.get_rect().origin;
            let relative = knob_orig - orig;

            this.main.set_rect(rect);
            this.main_knob.set_rect(Rectangle {
                origin: rect.origin + relative,
                extend: Delta::splat(1),
            });
        });

        let slider = PaletteHueSlider::new(
            self.main.get_rect().origin - Delta::new(0, HUE_HEIGHT as i32),
            &mut entry.single_fetch_mut().unwrap(),
        );
        entry.insert(slider);
    }
}
impl Palette {
    pub fn new(position: Position, interface: &mut Interface) -> Palette {
        let main = interface.create_painter(Rectangle {
            origin: position,
            extend: Delta::new(WIDTH as i32, HEIGHT as i32),
        });

        let main_knob = interface.create_wireframe(
            Rectangle {
                origin: position,
                extend: Delta::splat(1),
            },
            [1.0, 1.0, 1.0, 1.0],
        );

        main_knob.set_z_order(ZOrder::new(1));

        let mut palette = Palette {
            main,
            main_knob,
            hue: 0.0,
        };

        palette.redraw();

        palette
    }

    pub fn pick_color(&self) -> [u8; 4] {
        let base_position = self.main.get_rect().origin;
        let knob_position = self.main_knob.get_rect().origin;

        let x = knob_position.x - base_position.x;
        let y = knob_position.y - base_position.y;

        let x = x.rem_euclid(WIDTH as i32);
        let y = (HEIGHT as i32 - 1) - y.rem_euclid(HEIGHT as i32);

        let saturation = x as f32 / (WIDTH - 1) as f32;
        let lightness = 1.0 - (y as f32 / (HEIGHT - 1) as f32);
        let hsl: Hsl = Hsl::new(self.hue, saturation, lightness);
        let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

        [rgb.red, rgb.blue, rgb.green, 255]
    }

    fn redraw(&mut self) {
        let mut writer = self.main.open_writer();
        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                let saturation = x as f32 / (WIDTH - 1) as f32;
                let lightness = 1.0 - (y as f32 / (HEIGHT - 1) as f32);
                let hsl: Hsl = Hsl::new(self.hue, saturation, lightness);
                let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

                writer.write(x, y, [rgb.red, rgb.blue, rgb.green, 255]);
            }
        }
    }
}

pub struct PaletteHueSlider {
    hue: Painter,
    hue_knob: Wireframe,
}
impl Element for PaletteHueSlider {}
impl InsertElement for PaletteHueSlider {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<PointerHit>(move |event, entry| match event.0 {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = entry.fetch_mut::<PaletteHueSlider>(entry.handle()).unwrap();
                let rect = this.hue.get_rect();
                this.hue_knob.set_rect(Rectangle {
                    origin: Position::new(
                        point.x.clamp(rect.left(), rect.right() - 1),
                        rect.down(),
                    ),
                    extend: Delta::new(1, HUE_HEIGHT as i32),
                });

                if let Some(mut palette) = entry.single_fetch_mut::<Palette>() {
                    let base_position = this.hue.get_rect().left();
                    let knob_position = this.hue_knob.get_rect().left();

                    let x = (knob_position - base_position).rem_euclid(WIDTH as i32);

                    palette.hue = (x as f32 / WIDTH as f32) * -360.0;
                    palette.redraw();
                }
            }
            _ => (),
        });

        entry.getter::<PointerCollider>(|this| PointerCollider {
            rect: this.hue.get_rect(),
            z_order: this.hue.get_z_order(),
        });

        entry.getter::<Rectangle>(|this| this.hue.get_rect());

        entry.setter::<Rectangle>(|this, rect| {
            let orig = this.hue.get_rect().left();
            let knob_orig = this.hue_knob.get_rect().left();
            let relative = knob_orig - orig;

            this.hue.set_rect(rect);
            this.hue_knob.set_rect(Rectangle {
                origin: Position::new(rect.left() + relative, rect.down()),
                extend: Delta::new(1, HUE_HEIGHT as i32),
            });
        });
    }
}
impl PaletteHueSlider {
    fn new(position: Position, interface: &mut Interface) -> PaletteHueSlider {
        let mut hue = interface.create_painter(Rectangle {
            origin: position,
            extend: Delta::new(WIDTH as i32, HUE_HEIGHT as i32),
        });

        let mut writer = hue.open_writer();
        for x in 0..WIDTH {
            let hsl: Hsl = Hsl::new(x as f32 / WIDTH as f32 * -360.0, 0.8, 0.6);
            let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();
            for y in 0..HUE_HEIGHT {
                writer.write(x as i32, y as i32, [rgb.red, rgb.blue, rgb.green, 255]);
            }
        }

        drop(writer);

        let hue_knob = interface.create_wireframe(
            Rectangle {
                origin: position,
                extend: Delta::new(1, HUE_HEIGHT as i32),
            },
            [1.0, 1.0, 1.0, 1.0],
        );

        hue_knob.set_z_order(ZOrder::new(1));

        PaletteHueSlider { hue, hue_knob }
    }
}
