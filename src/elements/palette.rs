use palette::{FromColor, Hsl, Srgb, rgb::Rgb};

use crate::{
    elements::StrokeLayer,
    interface::{Interface, Painter, Redraw, Wireframe},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::pointer::{PointerCollider, PointerHit},
    world::{Element, Handle, World},
};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;
const HUE_HEIGHT: usize = 32;

pub struct Palette {
    main: Painter,
    main_knob: Wireframe,

    hue: Painter,
    hue_knob: Wireframe,

    redraw: bool,
}

impl Element for Palette {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        // main collider //

        let collider = world.insert(PointerCollider {
            rect: self.main.get_rect(),
            z_order: ZOrder::new(0),
        });

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = world.fetch_mut(this).unwrap();
                let rect = this.main.get_rect();
                this.main_knob.set_rect(Rectangle {
                    origin: point.clamp(Rectangle {
                        origin: rect.origin,
                        extend: rect.extend - Delta::splat(1),
                    }),
                    extend: Delta::splat(1),
                });

                let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
                layer.color = this.get_color();

                this.redraw = true;
            }
            _ => {}
        });

        world.dependency(collider, this);

        // hue collider //

        let collider = world.insert(PointerCollider {
            rect: self.hue.get_rect(),
            z_order: ZOrder::new(0),
        });

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Moved(point) | PointerEvent::Pressed(point) => {
                let mut this = world.fetch_mut(this).unwrap();
                let rect = this.hue.get_rect();
                this.hue_knob.set_rect(Rectangle {
                    origin: Position::new(
                        point.x.clamp(rect.left(), rect.right() - 1),
                        rect.down(),
                    ),
                    extend: Delta::new(1, HUE_HEIGHT as i32),
                });

                let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
                layer.color = this.get_color();

                this.redraw = true;
            }
            _ => {}
        });

        world.dependency(collider, this);

        // track redraw request //

        let interface = world.single::<Interface>().unwrap();

        let tracker = world.observer(interface, move |Redraw, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if this.redraw {
                this.redraw();
            }
        });

        world.dependency(tracker, this);
    }
}

impl Palette {
    pub fn new(position: Position, interface: &mut Interface) -> Palette {
        let main = Painter::new(
            Rectangle {
                origin: position,
                extend: Delta::new(WIDTH as i32, HEIGHT as i32),
            },
            interface,
        );

        let main_knob = Wireframe::new(
            Rectangle {
                origin: position,
                extend: Delta::splat(1),
            },
            [1.0, 1.0, 1.0, 1.0],
            interface,
        );

        main_knob.set_z_order(ZOrder::new(1));

        let hue = Painter::new(
            Rectangle {
                origin: position - Delta::new(0, HUE_HEIGHT as i32),
                extend: Delta::new(WIDTH as i32, HUE_HEIGHT as i32),
            },
            interface,
        );

        let hue_knob = Wireframe::new(
            Rectangle {
                origin: position - Delta::new(0, HUE_HEIGHT as i32),
                extend: Delta::new(1, HUE_HEIGHT as i32),
            },
            [1.0, 1.0, 1.0, 1.0],
            interface,
        );

        hue_knob.set_z_order(ZOrder::new(1));

        let mut palette = Palette {
            main,
            main_knob,
            hue,
            hue_knob,
            redraw: false,
        };

        palette.redraw();

        palette
    }

    pub fn get_color(&self) -> Srgb<u8> {
        Srgb::from_color(self.get_hsl()).into_format()
    }

    fn get_hsl(&self) -> Hsl {
        let base_position = self.hue.get_rect().left();
        let knob_position = self.hue_knob.get_rect().left();
        let hue = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let hue = (hue as f32 / WIDTH as f32) * 360.0;

        let base_position = self.main.get_rect().left();
        let knob_position = self.main_knob.get_rect().left();
        let saturation = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let saturation = saturation as f32 / WIDTH as f32;

        let base_position = self.main.get_rect().down();
        let knob_position = self.main_knob.get_rect().down();
        let lightness = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let lightness = lightness as f32 / HEIGHT as f32;

        Hsl::new(hue, saturation, lightness)
    }

    fn redraw(&mut self) {
        self.redraw = false;

        let (hue, saturation, lightness) = self.get_hsl().into_components();

        let mut writer = self.main.open_writer();
        for x in 0..WIDTH as i32 {
            for y in 0..HEIGHT as i32 {
                let saturation = x as f32 / WIDTH as f32;
                let lightness = 1.0 - (y as f32 / HEIGHT as f32);
                let hsl: Hsl = Hsl::new(hue, saturation, lightness);
                let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();

                writer.write(x, y, [rgb.red, rgb.green, rgb.blue, 255]);
            }
        }

        let mut writer = self.hue.open_writer();
        for x in 0..WIDTH {
            let hsl: Hsl = Hsl::new(x as f32 / WIDTH as f32 * 360.0, saturation, lightness);
            let rgb: Rgb<_, u8> = Rgb::from_color(hsl).into_format();
            for y in 0..HUE_HEIGHT {
                writer.write(x as i32, y as i32, [rgb.red, rgb.green, rgb.blue, 255]);
            }
        }
    }
}
