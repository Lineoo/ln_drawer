use glam::{I64Vec2, IVec2, UVec2, Vec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    color::{oklab::oklab_to_linear_srgb, srgb::mul_alpha},
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    tools::{collider::ToolRectCollider, pointer::PointerHit},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        common::WidgetCommon,
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
    pub oklab: Vec4,
    pub enabled: bool,
    pub camera: Handle<Camera>,
}

pub struct OklabBar {
    pub rect: Rectangle,
    pub oklab: Vec4,
    pub enabled: bool,
    pub camera: Handle<Camera>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OklabPolarMaterial {
    oklab: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OklabBarMaterial {
    oklab: Vec4,
}

pub struct ColorOklab(pub Vec4);
pub struct SetColorOklab(pub Vec4);

impl OklabPolar {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            camera: self.camera,
            material: OklabPolarMaterial { oklab: self.oklab },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: mul_alpha(oklab_to_linear_srgb(self.oklab)),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let collider = world.insert(ToolRectCollider {
            common: WidgetCommon {
                visible: self.enabled,
                camera: self.camera,
            },
            target: this.untyped(),
            rect: self.rect,
            order: 100,
        });

        world.dependency(quad, this);
        world.dependency(thumb, this);
        world.dependency(thumb_light, this);
        world.dependency(thumb_shadow, this);

        world.observer(this, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this);
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);
            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();

            this.oklab.y = (uv.x * 0.8 - 0.4).clamp(-0.4, 0.4);
            this.oklab.z = (uv.y * 0.8 - 0.4).clamp(-0.4, 0.4);
            this.oklab = clip_to_srgb_gamut(this.oklab);
            world.queue_trigger(this.handle(), ColorOklab(this.oklab));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this);
            this.enabled = enabled;
            world.queue_trigger(collider, SetWidgetVisible(enabled));
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            let thumb_rect = Rectangle::new_half(
                this.rect.origin
                    + (Vec2::new(this.oklab.y / 0.8 + 0.5, this.oklab.z / 0.8 + 0.5)
                        * rect.extend.as_vec2())
                    .as_ivec2(),
                UVec2::splat(THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
            world.queue_trigger(collider, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorOklab(oklab), world| {
            let mut this = world.fetch_mut(this);
            this.oklab = oklab;
            world.queue_trigger(quad, SetQuadMaterial(OklabPolarMaterial { oklab }));
            let thumb_rect = Rectangle::new_half(
                this.rect.origin
                    + (Vec2::new(oklab.y / 0.8 + 0.5, oklab.z / 0.8 + 0.5)
                        * this.rect.extend.as_vec2())
                    .as_ivec2(),
                UVec2::splat(THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(mul_alpha(oklab_to_linear_srgb(oklab))));
        });
    }
}

impl OklabBar {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            camera: self.camera,
            material: OklabBarMaterial { oklab: self.oklab },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: mul_alpha(oklab_to_linear_srgb(self.oklab)),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: self.enabled,
            camera: self.camera,
        });

        let collider = world.insert(ToolRectCollider {
            common: WidgetCommon {
                visible: self.enabled,
                camera: self.camera,
            },
            target: this.untyped(),
            rect: self.rect,
            order: 100,
        });

        world.observer(this, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this);
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);
            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();

            this.oklab.x = (uv.y).clamp(0.0, 1.0);
            this.oklab = clip_to_srgb_gamut(this.oklab);
            world.queue_trigger(this.handle(), ColorOklab(this.oklab));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this);
            this.enabled = enabled;
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
            world.queue_trigger(collider, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            let thumb_rect = Rectangle::new_half(
                IVec2::new(
                    rect.horizontal_center(),
                    rect.origin.y + (this.oklab.x * rect.extend.y as f32) as i32,
                ),
                UVec2::new(rect.extend.x / 2, THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
            world.queue_trigger(collider, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorOklab(oklab), world| {
            let mut this = world.fetch_mut(this);
            this.oklab = oklab;
            world.queue_trigger(quad, SetQuadMaterial(OklabBarMaterial { oklab }));
            let thumb_rect = Rectangle::new_half(
                IVec2::new(
                    this.rect.horizontal_center(),
                    this.rect.origin.y + (oklab.x * this.rect.extend.y as f32) as i32,
                ),
                UVec2::new(this.rect.extend.x / 2, THUMB_RADIUS as u32),
            );
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(mul_alpha(oklab_to_linear_srgb(oklab))));
        });

        world.dependency(quad, this);
        world.dependency(thumb, this);
        world.dependency(thumb_light, this);
        world.dependency(thumb_shadow, this);
    }
}

fn clip_to_srgb_gamut(oklab: Vec4) -> Vec4 {
    let l = oklab.x.clamp(0.0, 1.0);
    let (a, b) = (oklab.y, oklab.z);
    let c = (a * a + b * b).sqrt().max(1e-5);
    let t = find_gamut_intersection(a / c, b / c, l, c, l).min(1.0);
    Vec4::new(l, t * a, t * b, oklab.w)
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
