use palette::{FromColor, Hsl, SetHue, Srgba, WithAlpha};

use crate::{
    elements::{
        menu::{MenuDescriptor, MenuEntryDescriptor},
        stroke::StrokeLayer,
    },
    measures::{Position, Rectangle, Size},
    render::{
        LossyPrepare, RenderControl,
        canvas::{Canvas, CanvasDescriptor},
        wireframe::{Wireframe, WireframeDescriptor},
    },
    tools::pointer::{PointerCollider, PointerHit, PointerMenu},
    world::{Descriptor, Element, Handle, World},
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const HUE_HEIGHT: u32 = 32;

pub struct Palette {
    pub position: Position,
    hsl: Hsl,

    main: Handle<Canvas>,
    main_knob: Handle<Wireframe>,
    main_collider: Handle<PointerCollider>,

    hue: Handle<Canvas>,
    hue_knob: Handle<Wireframe>,
    hue_collider: Handle<PointerCollider>,

    redraw: bool,
    control: Handle<RenderControl>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PaletteDescriptor {
    pub position: Position,
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

impl Descriptor for PaletteDescriptor {
    type Target = Handle<Palette>;

    fn when_build(self, world: &World) -> Self::Target {
        let hx = (self.hue / 360.0 * WIDTH as f32).floor() as i32;
        let mx = (self.saturation * (WIDTH - 1) as f32).floor() as i32;
        let my = (self.lightness * (HEIGHT - 1) as f32).floor() as i32;

        // main //

        let main_rect = Rectangle {
            origin: self.position,
            extend: Size::new(WIDTH, HEIGHT),
        };

        let main = world.build(CanvasDescriptor {
            rect: main_rect,
            width: WIDTH,
            height: HEIGHT,
            visible: true,
            ..Default::default()
        });

        let main_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: self.position + Position::new(mx, my),
                extend: Size::splat(1),
            },
            order: 1,
            visible: true,
        });

        let main_collider = world.insert(PointerCollider {
            rect: main_rect,
            order: 0,
            enabled: true,
        });

        // hue //

        let hue_rect = Rectangle {
            origin: self.position + Position::new(0, -(HUE_HEIGHT as i32)),
            extend: Size::new(WIDTH, HUE_HEIGHT),
        };

        let hue = world.build(CanvasDescriptor {
            rect: hue_rect,
            width: WIDTH,
            height: HUE_HEIGHT,
            visible: true,
            ..Default::default()
        });

        let hue_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: self.position + Position::new(hx, -(HUE_HEIGHT as i32)),
                extend: Size::new(1, HUE_HEIGHT),
            },
            order: 1,
            visible: true,
        });

        let hue_collider = world.insert(PointerCollider {
            rect: hue_rect,
            order: 0,
            enabled: true,
        });

        let control = world.insert(RenderControl {
            visible: true,
            order: 1,
            refreshing: false,
        });

        world.insert(Palette {
            position: self.position,
            hsl: Hsl::new(self.hue, self.saturation, self.lightness),
            main,
            main_knob,
            main_collider,
            hue,
            hue_knob,
            hue_collider,
            redraw: true,
            control,
        })
    }
}

impl Element for Palette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        // main //

        world.observer(self.main_collider, move |event: &PointerHit, world, _| {
            let mut this = world.fetch_mut(this).unwrap();
            let main = world.fetch(this.main).unwrap();

            let cursor_position = event.position.clamp(Rectangle {
                origin: main.rect.origin,
                extend: main.rect.extend - Size::splat(1),
            });

            let saturation = (cursor_position.x - main.rect.left()).rem_euclid(WIDTH as i32);
            this.hsl.saturation = saturation as f32 / (WIDTH - 1) as f32;

            let lightness = (cursor_position.y - main.rect.down()).rem_euclid(WIDTH as i32);
            this.hsl.lightness = lightness as f32 / (HEIGHT - 1) as f32;

            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.color = this.get_color();

            this.redraw = true;
        });

        world.dependency(self.main, this);
        world.dependency(self.main_knob, this);
        world.dependency(self.main_collider, this);

        // hue //

        world.observer(self.hue_collider, move |event: &PointerHit, world, _| {
            let mut this = world.fetch_mut(this).unwrap();
            let hue = world.fetch(this.hue).unwrap();

            let cursor_position = (event.position.x).clamp(hue.rect.left(), hue.rect.right() - 1);

            let hue = (cursor_position - hue.rect.left()).rem_euclid(WIDTH as i32);
            this.hsl.set_hue((hue as f32 / WIDTH as f32) * 360.0);

            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.color = this.get_color();

            this.redraw = true;
        });

        world.dependency(self.hue, this);
        world.dependency(self.hue_knob, this);
        world.dependency(self.hue_collider, this);

        // menu //

        world.observer(
            self.main_collider,
            move |&PointerMenu(position), world, _| {
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
            },
        );

        world.observer(
            self.hue_collider,
            move |&PointerMenu(position), world, _| {
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
            },
        );

        // redraw //

        world.observer(self.control, move |LossyPrepare, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if !this.redraw {
                return;
            }

            this.redraw = false;

            let mut main = world.fetch_mut(this.main).unwrap();
            let mut writer = main.open_writer();
            for x in 0..WIDTH as i32 {
                for y in 0..HEIGHT as i32 {
                    let saturation = x as f32 / WIDTH as f32;
                    let lightness = 1.0 - (y as f32 / HEIGHT as f32);
                    let hsl = Hsl::new(this.hsl.hue, saturation, lightness);
                    let srgba = Srgba::from_color(hsl).with_alpha(1.0);

                    writer.write(x, y, srgba);
                }
            }

            let mut hue = world.fetch_mut(this.hue).unwrap();
            let mut writer = hue.open_writer();
            for x in 0..WIDTH {
                let hsl = Hsl::new(
                    x as f32 / WIDTH as f32 * 360.0,
                    this.hsl.saturation,
                    this.hsl.lightness,
                );
                let srgba = Srgba::from_color(hsl).with_alpha(1.0);
                for y in 0..HUE_HEIGHT {
                    writer.write(x as i32, y as i32, srgba);
                }
            }
        });

        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        let main_canvas = world.fetch(self.main).unwrap();
        let hue_canvas = world.fetch(self.hue).unwrap();
        let mut main_knob = world.fetch_mut(self.main_knob).unwrap();
        let mut hue_knob = world.fetch_mut(self.hue_knob).unwrap();

        let hue = (self.hsl.hue.into_positive_degrees() / 360.0 * WIDTH as f32).floor() as i32;
        let hx = hue_canvas.rect.left() + hue;

        let saturation = (self.hsl.saturation * (WIDTH - 1) as f32).floor() as i32;
        let mx = main_canvas.rect.left() + saturation;

        let lightness = (self.hsl.lightness * (HEIGHT - 1) as f32).floor() as i32;
        let my = main_canvas.rect.down() + lightness;

        let hr = hue_knob.rect;
        let mr = main_knob.rect;

        hue_knob.rect = Rectangle {
            origin: Position::new(hx, hr.origin.y),
            extend: hr.extend,
        };

        main_knob.rect = Rectangle {
            origin: Position::new(mx, my),
            extend: mr.extend,
        };
    }
}

impl Palette {
    pub fn to_descriptor(&self) -> PaletteDescriptor {
        let hsl = self.get_hsl();
        PaletteDescriptor {
            position: self.position,
            hue: hsl.hue.into_positive_degrees(),
            saturation: hsl.saturation,
            lightness: hsl.lightness,
        }
    }

    pub fn get_color(&self) -> Srgba {
        Srgba::from_color(self.get_hsl())
    }

    pub fn set_color(&mut self, color: Srgba) {
        self.set_hsl(Hsl::from_color(color));
        self.redraw = true;
    }

    fn get_hsl(&self) -> Hsl {
        self.hsl
    }

    fn set_hsl(&mut self, hsl: Hsl) {
        self.hsl = hsl;
    }
}
