use palette::{FromColor, Hsl, Srgba, WithAlpha};

use crate::{
    elements::{
        menu::{MenuDescriptor, MenuEntryDescriptor},
        stroke::StrokeLayer,
    },
    measures::{Position, Rectangle, Size},
    render::{
        RedrawPrepare, RenderControl,
        canvas::{Canvas, CanvasDescriptor},
        wireframe::{Wireframe, WireframeDescriptor},
    },
    tools::{
        pointer::{PointerCollider, PointerHit, PointerMenu},
        transform::{Transform, TransformUpdate},
    },
    world::{Descriptor, Element, Handle, World},
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const HUE_HEIGHT: u32 = 32;

pub struct Palette {
    main: Canvas,
    main_knob: Wireframe,

    hue: Canvas,
    hue_knob: Wireframe,

    redraw: bool,
}

#[derive(Debug, Default, bincode::Encode, bincode::Decode)]
pub struct PaletteDescriptor {
    pub position: Position,
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

impl Element for Palette {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        // main collider //

        let main_collider = world.insert(PointerCollider {
            rect: self.main.rect,
            order: 0,
        });

        world.observer(
            main_collider,
            move |event: &PointerHit, world, _| match event {
                PointerHit::Moving(point) | PointerHit::Pressed(point) => {
                    let mut this = world.fetch_mut(this).unwrap();
                    let rect = this.main.rect;
                    this.main_knob.rect = Rectangle {
                        origin: point.clamp(Rectangle {
                            origin: rect.origin,
                            extend: rect.extend - Size::splat(1),
                        }),
                        extend: Size::splat(1),
                    };

                    let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
                    layer.color = this.get_color();

                    this.main_knob.upload();
                    this.redraw = true;
                }
                _ => {}
            },
        );

        world.dependency(main_collider, this);

        // hue collider //

        let hue_collider = world.insert(PointerCollider {
            rect: self.hue.rect,
            order: 0,
        });

        world.observer(
            hue_collider,
            move |event: &PointerHit, world, _| match event {
                PointerHit::Moving(point) | PointerHit::Pressed(point) => {
                    let mut this = world.fetch_mut(this).unwrap();
                    let rect = this.hue.rect;
                    this.hue_knob.rect = Rectangle {
                        origin: Position::new(
                            point.x.clamp(rect.left(), rect.right() - 1),
                            rect.down(),
                        ),
                        extend: Size::new(1, HUE_HEIGHT),
                    };

                    let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
                    layer.color = this.get_color();

                    this.hue_knob.upload();
                    this.redraw = true;
                }
                _ => {}
            },
        );

        world.dependency(hue_collider, this);

        // menu //

        world.observer(main_collider, move |&PointerMenu(position), world, _| {
            world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world, _| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            });
        });

        world.observer(hue_collider, move |&PointerMenu(position), world, _| {
            world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world, _| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            });
        });

        // redraw //

        let control = world.insert(RenderControl {
            visible: true,
            order: 1,
        });

        world.observer(control, move |RedrawPrepare, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if this.redraw {
                this.redraw();
            }
        });

        world.dependency(control, this);

        // transform //

        let transform = world.insert(Transform {
            rect: self.main.rect,
            resizable: false,
        });

        world.observer(transform, move |TransformUpdate, world, transform| {
            let mut fetched = world.fetch_mut(this).unwrap();
            let mut main_collider = world.fetch_mut(main_collider).unwrap();
            let mut hue_collider = world.fetch_mut(hue_collider).unwrap();
            let transform = world.fetch(transform).unwrap();

            let delta = transform.rect.left_down() - fetched.main.rect.left_down();

            fetched.main.rect += delta;
            fetched.main_knob.rect += delta;
            fetched.hue.rect += delta;
            fetched.hue_knob.rect += delta;

            main_collider.rect += delta;
            hue_collider.rect += delta;
        });

        world.dependency(transform, this);
    }
}

impl Descriptor for PaletteDescriptor {
    type Target = Handle<Palette>;

    fn build(self, world: &World) -> Self::Target {
        let main = world.build(CanvasDescriptor {
            rect: Rectangle {
                origin: self.position,
                extend: Size::new(WIDTH, HEIGHT),
            },
            width: WIDTH,
            height: HEIGHT,
            visible: true,
            ..Default::default()
        });

        let main_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: self.position,
                extend: Size::splat(1),
            },
            order: 1,
            visible: true,
        });

        let hue = world.build(CanvasDescriptor {
            rect: Rectangle {
                origin: Position::new(
                    self.position.x,
                    self.position.y.wrapping_sub_unsigned(HUE_HEIGHT),
                ),
                extend: Size::new(WIDTH, HUE_HEIGHT),
            },
            width: WIDTH,
            height: HUE_HEIGHT,
            visible: true,
            ..Default::default()
        });

        let hue_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: Position::new(
                    self.position.x,
                    self.position.y.wrapping_sub_unsigned(HUE_HEIGHT),
                ),
                extend: Size::new(1, HUE_HEIGHT),
            },
            order: 1,
            visible: true,
        });

        let mut palette = Palette {
            main,
            main_knob,
            hue,
            hue_knob,
            redraw: false,
        };

        palette.set_hsl(Hsl::new(self.hue, self.saturation, self.lightness));
        palette.redraw();

        world.insert(palette)
    }
}

impl Palette {
    pub fn to_descriptor(&self) -> PaletteDescriptor {
        let hsl = self.get_hsl();
        PaletteDescriptor {
            position: self.main.rect.left_down(),
            hue: hsl.hue.into_degrees(),
            saturation: hsl.saturation,
            lightness: hsl.lightness,
        }
    }

    pub fn get_color(&self) -> Srgba {
        Srgba::from_color(self.get_hsl())
    }

    pub fn set_color(&mut self, color: Srgba) {
        self.set_hsl(Hsl::from_color(color));
    }

    fn get_hsl(&self) -> Hsl {
        let base_position = self.hue.rect.left();
        let knob_position = self.hue_knob.rect.left();
        let hue = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let hue = (hue as f32 / WIDTH as f32) * 360.0;

        let base_position = self.main.rect.left();
        let knob_position = self.main_knob.rect.left();
        let saturation = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let saturation = saturation as f32 / WIDTH as f32;

        let base_position = self.main.rect.down();
        let knob_position = self.main_knob.rect.down();
        let lightness = (knob_position - base_position).rem_euclid(WIDTH as i32);
        let lightness = lightness as f32 / HEIGHT as f32;

        Hsl::new(hue, saturation, lightness)
    }

    fn set_hsl(&mut self, hsl: Hsl) {
        let (hue, saturation, lightness) = hsl.into_components();

        let hue = (hue.into_degrees() / 360.0 * WIDTH as f32).floor() as i32;
        let hx = self.hue.rect.left() + hue;

        let saturation = (saturation * WIDTH as f32).floor() as i32;
        let mx = self.main.rect.left() + saturation;

        let lightness = (lightness * HEIGHT as f32).floor() as i32;
        let my = self.main.rect.down() + lightness;

        let hr = self.hue_knob.rect;
        let mr = self.main_knob.rect;

        self.hue_knob.rect = Rectangle {
            origin: Position::new(hx, hr.origin.y),
            extend: hr.extend,
        };

        self.main_knob.rect = Rectangle {
            origin: Position::new(mx, my),
            extend: mr.extend,
        };

        self.hue_knob.upload();
        self.main_knob.upload();

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
                let hsl = Hsl::new(hue, saturation, lightness);
                let srgba = Srgba::from_color(hsl).with_alpha(1.0);

                writer.write(x, y, srgba);
            }
        }

        let mut writer = self.hue.open_writer();
        for x in 0..WIDTH {
            let hsl = Hsl::new(x as f32 / WIDTH as f32 * 360.0, saturation, lightness);
            let srgba = Srgba::from_color(hsl).with_alpha(1.0);
            for y in 0..HUE_HEIGHT {
                writer.write(x as i32, y as i32, srgba);
            }
        }
    }
}
