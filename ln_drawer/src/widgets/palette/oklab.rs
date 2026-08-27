use glam::{I64Vec2, IVec2, UVec2, Vec2, Vec3A};
use ln_world::{Element, Handle, World};
use palette::{IntoColor, Oklab, Srgba};

use crate::{
    measures::{FI64Ext, Rectangle},
    tools::{collider::ToolCollider, pointer::PointerHit},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        palette::utils::find_gamut_intersection,
        renderer::{
            quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
            rrect::{RRect, SetRRectColor},
        },
        shaders::shader_compile,
    },
};

const THUMB_RADIUS: f32 = 5.0;

pub struct OklabPolar {
    pub rect: Rectangle,
    pub color: Oklab,
    pub enabled: bool,
}

pub struct OklabBar {
    pub rect: Rectangle,
    pub color: Oklab,
    pub enabled: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OklabPolarMaterial {
    oklab: Vec3A,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OklabBarMaterial {
    oklab: Vec3A,
}

pub struct ColorOklab(pub Oklab);
pub struct SetColorOklab(pub Oklab);

impl OklabPolar {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: OklabPolarMaterial {
                oklab: Vec3A::new(self.color.l, self.color.a, self.color.b),
            },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: self.color.into_color(),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: true,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Srgba::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: true,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Srgba::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: true,
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 100,
            enabled: self.enabled,
        });

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);
            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();

            this.color.a = (uv.x * 0.8 - 0.4).clamp(-0.4, 0.4);
            this.color.b = (uv.y * 0.8 - 0.4).clamp(-0.4, 0.4);
            this.color = clip_to_srgb_gamut(this.color);
            world.queue_trigger(this.handle(), ColorOklab(this.color));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.enabled = enabled;
            collider.enabled = enabled;
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            collider.rect = rect;
            let thumb_rect = Rectangle::new_half(
                this.rect.origin
                    + (Vec2::new(this.color.a / 0.8 + 0.5, this.color.b / 0.8 + 0.5)
                        * rect.extend.as_vec2())
                    .as_ivec2(),
                UVec2::splat(THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorOklab(color), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.color = color;
            world.queue_trigger(
                quad,
                SetQuadMaterial(OklabPolarMaterial {
                    oklab: Vec3A::new(color.l, color.a, color.b),
                }),
            );
            let thumb_rect = Rectangle::new_half(
                this.rect.origin
                    + (Vec2::new(color.a / 0.8 + 0.5, color.b / 0.8 + 0.5)
                        * this.rect.extend.as_vec2())
                    .as_ivec2(),
                UVec2::splat(THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(color.into_color()));
        });

        world.dependency(quad, this);
    }
}

impl OklabBar {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: OklabBarMaterial {
                oklab: Vec3A::new(self.color.l, self.color.a, self.color.b),
            },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: self.color.into_color(),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: true,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Srgba::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: true,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Srgba::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: true,
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 100,
            enabled: self.enabled,
        });

        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);
            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();

            this.color.l = (uv.y).clamp(0.0, 1.0);
            this.color = clip_to_srgb_gamut(this.color);
            world.queue_trigger(this.handle(), ColorOklab(this.color));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.enabled = enabled;
            collider.enabled = enabled;
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            collider.rect = rect;
            let thumb_rect = Rectangle::new_half(
                IVec2::new(
                    rect.horizontal_center(),
                    rect.origin.y + (this.color.l * rect.extend.y as f32) as i32,
                ),
                UVec2::new(rect.extend.x / 2, THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorOklab(color), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.color = color;
            world.queue_trigger(
                quad,
                SetQuadMaterial(OklabBarMaterial {
                    oklab: Vec3A::new(color.l, color.a, color.b),
                }),
            );
            let thumb_rect = Rectangle::new_half(
                IVec2::new(
                    this.rect.horizontal_center(),
                    this.rect.origin.y + (color.l * this.rect.extend.y as f32) as i32,
                ),
                UVec2::new(this.rect.extend.x / 2, THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(color.into_color()));
        });

        world.dependency(quad, this);
    }
}

fn clip_to_srgb_gamut(color: Oklab) -> Oklab {
    let l = color.l.clamp(0.0, 1.0);
    let (b, a) = (color.b, color.a);
    let c = (a * a + b * b).sqrt().max(1e-5);
    let t = find_gamut_intersection(a / c, b / c, l, c, l).min(1.0);
    Oklab::new(l, t * a, t * b)
}

impl QuadMaterial for OklabPolarMaterial {
    fn label() -> &'static str {
        "palette_oklab_polar"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("oklab_polar.wgsl"), &[]).into())
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl QuadMaterial for OklabBarMaterial {
    fn label() -> &'static str {
        "palette_oklab_bar"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("oklab_bar.wgsl"), &[]).into())
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for OklabPolar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for OklabBar {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
