use palette::{FromColor, Hsl, SetHue, Srgba};

use crate::{
    measures::{Position, Rectangle, Size},
    render::{
        RenderControl,
        rectangle::{RectangleMesh, RectangleMeshDescriptor, RectangleMeshMaterial},
        wireframe::{Wireframe, WireframeDescriptor},
    },
    stroke::StrokeLayer,
    tools::{collider::ToolCollider, pointer::PointerHit},
    world::{Descriptor, Element, Handle, World},
};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const HUE_HEIGHT: u32 = 32;

pub struct Palette {
    pub position: Position,
    hsl: Hsl,

    main: Handle<RectangleMesh<PaletteMain>>,
    main_knob: Handle<Wireframe>,
    main_collider: Handle<ToolCollider>,

    hue: Handle<RectangleMesh<PaletteHue>>,
    hue_knob: Handle<Wireframe>,
    hue_collider: Handle<ToolCollider>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PaletteDescriptor {
    pub position: Position,
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteMain {
    h: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteHue {
    s: f32,
    l: f32,
}

impl RectangleMeshMaterial for PaletteMain {
    fn label() -> &'static str {
        "palette"
    }

    fn fragment() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(include_str!("palette.wgsl").into())
    }

    fn entry_point() -> Option<&'static str> {
        Some("palette_main")
    }
}

impl RectangleMeshMaterial for PaletteHue {
    fn label() -> &'static str {
        "palette"
    }

    fn fragment() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(include_str!("palette.wgsl").into())
    }

    fn entry_point() -> Option<&'static str> {
        Some("palette_hue")
    }
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

        let main = world.build(RectangleMeshDescriptor {
            rect: main_rect,
            visible: true,
            order: 1,
            material: PaletteMain { h: 20.0, _pad: 0.0 },
        });

        let main_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: self.position + Position::new(mx, my),
                extend: Size::splat(1),
            },
            order: 1,
            visible: true,
        });

        let main_collider = world.insert(ToolCollider {
            rect: main_rect,
            order: 0,
            enabled: true,
        });

        // hue //

        let hue_rect = Rectangle {
            origin: self.position + Position::new(0, -(HUE_HEIGHT as i32)),
            extend: Size::new(WIDTH, HUE_HEIGHT),
        };

        let hue = world.build(RectangleMeshDescriptor {
            rect: hue_rect,
            visible: true,
            order: 1,
            material: PaletteHue { s: 1.0, l: 0.5 },
        });

        let hue_knob = world.build(WireframeDescriptor {
            rect: Rectangle {
                origin: self.position + Position::new(hx, -(HUE_HEIGHT as i32)),
                extend: Size::new(1, HUE_HEIGHT),
            },
            order: 1,
            visible: true,
        });

        let hue_collider = world.insert(ToolCollider {
            rect: hue_rect,
            order: 0,
            enabled: true,
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
        })
    }
}

impl Element for Palette {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        // main //

        world.observer(self.main_collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut hue = world.fetch_mut(this.hue).unwrap();
            let main = world.fetch(this.main).unwrap();

            let cursor_position = event.position.clamp(Rectangle {
                origin: main.desc.rect.origin,
                extend: main.desc.rect.extend - Size::splat(1),
            });

            let saturation = (cursor_position.x - main.desc.rect.left()).rem_euclid(WIDTH as i32);
            hue.desc.material.s = saturation as f32 / (WIDTH - 1) as f32;
            this.hsl.saturation = hue.desc.material.s;

            let lightness = (cursor_position.y - main.desc.rect.down()).rem_euclid(WIDTH as i32);
            hue.desc.material.l = lightness as f32 / (HEIGHT - 1) as f32;
            this.hsl.lightness = hue.desc.material.l;

            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.front_color = this.get_color();
        });

        world.dependency(self.main, this);
        world.dependency(self.main_knob, this);
        world.dependency(self.main_collider, this);

        // hue //

        world.observer(self.hue_collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut main = world.fetch_mut(this.main).unwrap();
            let hue = world.fetch(this.hue).unwrap();

            let cursor_position =
                (event.position.x).clamp(hue.desc.rect.left(), hue.desc.rect.right() - 1);

            let hue = (cursor_position - hue.desc.rect.left()).rem_euclid(WIDTH as i32);
            main.desc.material.h = hue as f32 / WIDTH as f32;
            this.hsl.set_hue(main.desc.material.h * 360.0);

            let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
            layer.front_color = this.get_color();
        });

        world.dependency(self.hue, this);
        world.dependency(self.hue_knob, this);
        world.dependency(self.hue_collider, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        let main_canvas = world.fetch(self.main).unwrap();
        let hue_canvas = world.fetch(self.hue).unwrap();
        let mut main_knob = world.fetch_mut(self.main_knob).unwrap();
        let mut hue_knob = world.fetch_mut(self.hue_knob).unwrap();

        let hue = (self.hsl.hue.into_positive_degrees() / 360.0 * WIDTH as f32).floor() as i32;
        let hx = hue_canvas.desc.rect.left() + hue;

        let saturation = (self.hsl.saturation * (WIDTH - 1) as f32).floor() as i32;
        let mx = main_canvas.desc.rect.left() + saturation;

        let lightness = (self.hsl.lightness * (HEIGHT - 1) as f32).floor() as i32;
        let my = main_canvas.desc.rect.down() + lightness;

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
    }

    fn get_hsl(&self) -> Hsl {
        self.hsl
    }

    fn set_hsl(&mut self, hsl: Hsl) {
        self.hsl = hsl;
    }
}
