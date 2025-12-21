use palette::{FromColor, Hsl, Srgb, rgb::Rgb};

use crate::{
    elements::{
        menu::{MenuDescriptor, MenuEntryDescriptor},
        stroke::StrokeLayer,
    },
    interface::{Interface, Painter, Redraw, Wireframe},
    lnwin::PointerEvent,
    measures::{Delta, Position, Rectangle, ZOrder},
    tools::{
        pointer::{PointerCollider, PointerHit, PointerMenu},
        transform::{Transform, TransformUpdate},
    },
    world::{Element, ElementDescriptor, Handle, World},
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

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct PaletteDescriptor {
    position: Position,
    hue: f32,
    saturation: f32,
    lightness: f32,
}

impl Element for Palette {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        // main collider //

        let main_collider = world.insert(PointerCollider {
            rect: self.main.get_rect(),
            z_order: ZOrder::new(0),
        });

        world.observer(
            main_collider,
            move |&PointerHit(event), world, _| match event {
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
            },
        );

        world.dependency(main_collider, this);

        // hue collider //

        let hue_collider = world.insert(PointerCollider {
            rect: self.hue.get_rect(),
            z_order: ZOrder::new(0),
        });

        world.observer(
            hue_collider,
            move |&PointerHit(event), world, _| match event {
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
            },
        );

        world.dependency(hue_collider, this);

        // track redraw request //

        let interface = world.single::<Interface>().unwrap();

        let tracker = world.observer(interface, move |Redraw, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if this.redraw {
                this.redraw();
            }
        });

        world.dependency(tracker, this);

        // menu //

        world.observer(main_collider, move |&PointerMenu(position), world, _| {
            world.insert(world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            }));
        });

        world.observer(hue_collider, move |&PointerMenu(position), world, _| {
            world.insert(world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            }));
        });

        // transform //

        let transform = world.insert(Transform {
            rect: self.main.get_rect(),
            resizable: false,
        });

        world.observer(transform, move |TransformUpdate, world, transform| {
            let mut fetched = world.fetch_mut(this).unwrap();
            let mut main_collider = world.fetch_mut(main_collider).unwrap();
            let mut hue_collider = world.fetch_mut(hue_collider).unwrap();
            let transform = world.fetch(transform).unwrap();

            let delta = transform.rect.left_down() - fetched.main.get_rect().left_down();

            let dest = fetched.main.get_rect() + delta;
            fetched.main.set_rect(dest);

            let dest = fetched.main_knob.get_rect() + delta;
            fetched.main_knob.set_rect(dest);

            let dest = fetched.hue.get_rect() + delta;
            fetched.hue.set_rect(dest);

            let dest = fetched.hue_knob.get_rect() + delta;
            fetched.hue_knob.set_rect(dest);

            main_collider.rect += delta;
            hue_collider.rect += delta;
        });

        world.dependency(transform, this);
    }
}

impl ElementDescriptor for PaletteDescriptor {
    type Target = Palette;

    fn build(self, world: &World) -> Self::Target {
        Palette::new(self, &mut world.single_fetch_mut().unwrap())
    }
}

impl Palette {
    pub fn new(descriptor: PaletteDescriptor, interface: &mut Interface) -> Palette {
        let main = Painter::new_empty(
            Rectangle {
                origin: descriptor.position,
                extend: Delta::new(WIDTH as i32, HEIGHT as i32),
            },
            interface,
        );

        let main_knob = Wireframe::new(
            Rectangle {
                origin: descriptor.position,
                extend: Delta::splat(1),
            },
            [1.0, 1.0, 1.0, 1.0],
            interface,
        );

        main_knob.set_z_order(ZOrder::new(1));

        let hue = Painter::new_empty(
            Rectangle {
                origin: descriptor.position - Delta::new(0, HUE_HEIGHT as i32),
                extend: Delta::new(WIDTH as i32, HUE_HEIGHT as i32),
            },
            interface,
        );

        let hue_knob = Wireframe::new(
            Rectangle {
                origin: descriptor.position - Delta::new(0, HUE_HEIGHT as i32),
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

        palette.set_hsl(Hsl::new(
            descriptor.hue,
            descriptor.saturation,
            descriptor.lightness,
        ));

        palette.redraw();

        palette
    }

    pub fn to_descriptor(&self) -> PaletteDescriptor {
        let hsl = self.get_hsl();
        PaletteDescriptor {
            position: self.main.get_rect().left_down(),
            hue: hsl.hue.into_degrees(),
            saturation: hsl.saturation,
            lightness: hsl.lightness,
        }
    }

    pub fn get_color(&self) -> Srgb<u8> {
        Srgb::from_color(self.get_hsl()).into_format()
    }

    pub fn set_color(&mut self, color: Srgb<u8>) {
        self.set_hsl(Hsl::from_color(color.into_format()));
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

    fn set_hsl(&mut self, hsl: Hsl) {
        let (hue, saturation, lightness) = hsl.into_components();

        let hue = (hue.into_degrees() / 360.0 * WIDTH as f32).floor() as i32;
        let hx = (self.hue.get_rect().left() + hue).rem_euclid(WIDTH as i32);

        let saturation = (saturation * WIDTH as f32).floor() as i32;
        let mx = (self.main.get_rect().left() + saturation).rem_euclid(WIDTH as i32);

        let lightness = (lightness * HEIGHT as f32).floor() as i32;
        let my = (self.main.get_rect().down() + lightness).rem_euclid(HEIGHT as i32);

        let hr = self.hue_knob.get_rect();
        let mr = self.main_knob.get_rect();

        self.hue_knob.set_rect(Rectangle {
            origin: Position::new(hx, hr.origin.y),
            extend: hr.extend,
        });

        self.main_knob.set_rect(Rectangle {
            origin: Position::new(mx, my),
            extend: mr.extend,
        });

        self.redraw = true;
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
