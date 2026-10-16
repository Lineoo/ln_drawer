use std::f32::consts::TAU;

use glam::{I64Vec2, UVec2, Vec2, Vec4};
use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::{
    color::{
        hsl::hsl_to_srgb,
        srgb::{mul_alpha, srgb_gamma_decode},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    tools::{
        collider::{ToolControl, ToolControlUpdate, ToolHitResult, ToolRectCollider},
        pointer::PointerHit,
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        common::WidgetCommon,
        renderer::{
            quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
            rrect::{RRect, SetRRectColor},
        },
        shaders::shader_compile,
    },
};

/// Inner radius of the hue ring, normalized so that the outer radius is
/// [`OUTER_RADIUS`]. The saturation-lightness square is inscribed in it.
pub const INNER_RADIUS: f32 = 0.4;

/// Outer radius of the hue ring, normalized so that it touches the panel rect.
pub const OUTER_RADIUS: f32 = 0.5;

const THUMB_RADIUS: f32 = 5.0;

/// Saturation-lightness square panel whose x axis stands for saturation and
/// y axis stands for lightness.
pub struct HslSquare {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub hsl: Vec4,
}

/// Hue ring panel. The hue is picked on the annulus between
/// [`HslRing::inner_radius`] and [`OUTER_RADIUS`].
pub struct HslRing {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub hsl: Vec4,
    pub inner_radius: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HslSquareMaterial {
    hsl: Vec4,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HslRingMaterial {
    hsl: Vec4,
    inner_radius: f32,
    _pad: [u32; 3],
}

/// Ring shaped collider which hits the annulus between two radii. The radii are
/// normalized in the same unit as the panel quads, where `0.5` touches the rect.
pub struct ToolRingCollider {
    pub common: WidgetCommon,
    pub target: HandleAny,
    pub rect: Rectangle,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub order: isize,
}

pub struct ColorHsla(pub Vec4);
pub struct SetColorHsla(pub Vec4);

fn thumb_color(hsl: Vec4) -> Vec4 {
    mul_alpha(srgb_gamma_decode(hsl_to_srgb(hsl)))
}

fn square_thumb(rect: Rectangle, hsl: Vec4) -> Rectangle {
    let center = rect.origin + (Vec2::new(hsl.y, hsl.z) * rect.extend.as_vec2()).as_ivec2();
    Rectangle::new_half(center, UVec2::splat(THUMB_RADIUS as u32))
}

fn ring_thumb(rect: Rectangle, hsl: Vec4, inner_radius: f32) -> Rectangle {
    let angle = hsl.x * TAU;
    let radius = (inner_radius + OUTER_RADIUS) * 0.5;
    let uv = Vec2::splat(0.5) + Vec2::new(angle.cos(), angle.sin()) * radius;
    let center = rect.origin + (uv * rect.extend.as_vec2()).as_ivec2();
    Rectangle::new_half(center, UVec2::splat(THUMB_RADIUS as u32))
}

impl HslSquare {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.common.visible,
            order: 60,
            camera: self.common.camera,
            material: HslSquareMaterial { hsl: self.hsl },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: thumb_color(self.hsl),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let collider = world.insert(ToolRectCollider {
            common: self.common,
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

            this.hsl.y = uv.x.clamp(0.0, 1.0);
            this.hsl.z = uv.y.clamp(0.0, 1.0);
            world.queue_trigger(this.handle(), ColorHsla(this.hsl));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = enabled;
            world.queue_trigger(collider, SetWidgetVisible(enabled));
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            let thumb_rect = square_thumb(rect, this.hsl);
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
            world.queue_trigger(collider, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorHsla(hsl), world| {
            let mut this = world.fetch_mut(this);
            this.hsl = hsl;
            world.queue_trigger(quad, SetQuadMaterial(HslSquareMaterial { hsl }));
            let thumb_rect = square_thumb(this.rect, hsl);
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(thumb_color(hsl)));
        });
    }
}

impl HslRing {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.common.visible,
            order: 60,
            camera: self.common.camera,
            material: HslRingMaterial {
                hsl: self.hsl,
                inner_radius: self.inner_radius,
                _pad: [0; 3],
            },
        });

        let thumb = world.insert(RRect {
            rect: Rectangle::default(),
            order: 150,
            color: thumb_color(self.hsl),
            radius: THUMB_RADIUS,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let thumb_light = world.insert(RRect {
            rect: Rectangle::default(),
            order: 140,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let thumb_shadow = world.insert(RRect {
            rect: Rectangle::default(),
            order: 130,
            color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
            enabled: self.common.visible,
            camera: self.common.camera,
        });

        let collider = world.insert(ToolRingCollider {
            common: self.common,
            target: this.untyped(),
            rect: self.rect,
            inner_radius: self.inner_radius,
            outer_radius: OUTER_RADIUS,
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
            let delta = uv - Vec2::splat(0.5);

            this.hsl.x = (delta.y.atan2(delta.x) / TAU).rem_euclid(1.0);
            world.queue_trigger(this.handle(), ColorHsla(this.hsl));
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = enabled;
            world.queue_trigger(collider, SetWidgetVisible(enabled));
            world.queue_trigger(thumb, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_light, SetWidgetVisible(enabled));
            world.queue_trigger(thumb_shadow, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            let thumb_rect = ring_thumb(rect, this.hsl, this.inner_radius);
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
            world.queue_trigger(collider, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorHsla(hsl), world| {
            let mut this = world.fetch_mut(this);
            this.hsl = hsl;
            world.queue_trigger(
                quad,
                SetQuadMaterial(HslRingMaterial {
                    hsl,
                    inner_radius: this.inner_radius,
                    _pad: [0; 3],
                }),
            );
            let thumb_rect = ring_thumb(this.rect, hsl, this.inner_radius);
            world.queue_trigger(thumb, SetWidgetRectangle(thumb_rect));
            world.queue_trigger(thumb_light, SetWidgetRectangle(thumb_rect.expand(1)));
            world.queue_trigger(thumb_shadow, SetWidgetRectangle(thumb_rect.expand(2)));
            world.queue_trigger(thumb, SetRRectColor(thumb_color(hsl)));
        });
    }
}

impl QuadMaterial for HslSquareMaterial {
    fn label() -> &'static str {
        "palette_hsl_square"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("hsl_square.wgsl"), &[]).into())
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl QuadMaterial for HslRingMaterial {
    fn label() -> &'static str {
        "palette_hsl_ring"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("hsl_ring.wgsl"), &[]).into())
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }

    fn edge(&self) -> UVec2 {
        UVec2::splat(2)
    }
}

impl Element for HslSquare {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for HslRing {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for ToolRingCollider {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(ToolControl {
            common: self.common,
            order: self.order,
            hit: Box::new(move |world, screen| {
                let this = world.fetch(this);
                let camera = world.fetch(this.common.camera);
                let position = camera.screen_to_world(screen);
                let delta = (position.q32_floor() - this.rect.origin).as_vec2()
                    / this.rect.extend.as_vec2()
                    - Vec2::splat(0.5);
                let radius = delta.length();

                if radius >= this.inner_radius && radius <= this.outer_radius {
                    Some(ToolHitResult {
                        camera: this.common.camera,
                        handle: this.target,
                    })
                } else {
                    None
                }
            }),
        });

        world.dependency(control, this);
        world.dependency(this, self.common.camera);
        world.dependency(this, self.target);
        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = visible;
            world.queue_trigger(control, SetWidgetVisible(visible));
        });
        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            world.queue_trigger(world.single::<Lnwindow>(), ToolControlUpdate);
        });
    }
}
