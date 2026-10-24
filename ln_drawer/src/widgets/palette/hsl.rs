use std::f32::consts::TAU;

use glam::{I64Vec2, UVec2, Vec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    color::{
        hsl::hsl_to_srgb,
        srgb::{mul_alpha, srgb_gamma_decode},
    },
    layout::{
        transfer::Transfer,
        val::{Val, ValChanged},
    },
    measures::{FI64Ext, Rectangle},
    tools::{
        collider::{ToolRectCollider, ToolRingCollider},
        pointer::PointerHit,
    },
    widgets::{
        common::WidgetCommon,
        renderer::{
            quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
            rrect::RRect,
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
    pub rect: Handle<Val<Rectangle>>,
    pub hsl: Vec4,
}

/// Hue ring panel. The hue is picked on the annulus between
/// [`HslRing::inner_radius`] and [`OUTER_RADIUS`].
pub struct HslRing {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
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
        world.scope("hsl_square");
        world.set_autodeps(Some(this));

        let thumb_rect = Transfer::derive(world, self.rect).build_with(move |&rect, world| {
            let this = world.fetch(this);
            square_thumb(rect, this.hsl)
        });
        let thumb_light_rect = Transfer::derive(world, thumb_rect).build(|rect| rect.expand(1));
        let thumb_shadow_rect = Transfer::derive(world, thumb_rect).build(|rect| rect.expand(2));

        let quad = world.insert(QuadMesh {
            common: self.common,
            rect: self.rect,
            order: 60,
            material: HslSquareMaterial { hsl: self.hsl },
        });

        let thumb_color_val = world.insert(Val::new(thumb_color(self.hsl)));
        world.insert(RRect {
            common: self.common,
            rect: thumb_rect,
            order: 150,
            color: thumb_color_val,
            radius: THUMB_RADIUS,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: thumb_light_rect,
            order: 140,
            color: world.insert(Val::new(Vec4::new(1.0, 1.0, 1.0, 1.0))),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: thumb_shadow_rect,
            order: 130,
            color: world.insert(Val::new(Vec4::new(0.0, 0.0, 0.0, 1.0))),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
        });

        world.insert(ToolRectCollider {
            common: self.common,
            target: this.untyped(),
            rect: self.rect,
            order: 100,
        });

        world.observer(this, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;
            let delta = event.position - I64Vec2::q32_from_i32(rect.origin);
            let uv = (delta.q32_as_f64() / rect.extend.as_dvec2()).as_vec2();

            this.hsl.y = uv.x.clamp(0.0, 1.0);
            this.hsl.z = uv.y.clamp(0.0, 1.0);
            world.queue_trigger(this.handle(), ColorHsla(this.hsl));
        });

        world.observer(this, move |&SetColorHsla(hsl), world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;
            let mut thumb_rect = world.fetch_mut(thumb_rect);
            let mut thumb_color_val = world.fetch_mut(thumb_color_val);
            this.hsl = hsl;
            thumb_rect.val = square_thumb(rect, this.hsl);
            thumb_color_val.val = thumb_color(hsl);
            world.queue_trigger(thumb_rect.handle(), ValChanged);
            world.queue_trigger(thumb_color_val.handle(), ValChanged);
            world.queue_trigger(quad, SetQuadMaterial(this.mat()));
        });
    }

    fn mat(&self) -> HslSquareMaterial {
        HslSquareMaterial { hsl: self.hsl }
    }
}

impl HslRing {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        world.scope("hsl_ring");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let thumb_rect = Transfer::derive(world, self.rect).build_with(move |&rect, world| {
            let this = world.fetch(this);
            ring_thumb(rect, this.hsl, this.inner_radius)
        });
        let thumb_light_rect = Transfer::derive(world, thumb_rect).build(|rect| rect.expand(1));
        let thumb_shadow_rect = Transfer::derive(world, thumb_rect).build(|rect| rect.expand(2));

        let quad = world.insert(QuadMesh {
            common: self.common,
            rect: self.rect,
            order: 60,
            material: HslRingMaterial {
                hsl: self.hsl,
                inner_radius: self.inner_radius,
                _pad: [0; 3],
            },
        });

        let thumb_color_val = world.insert(Val::new(thumb_color(self.hsl)));
        world.insert(RRect {
            common: self.common,
            rect: thumb_rect,
            order: 150,
            color: thumb_color_val,
            radius: THUMB_RADIUS,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: thumb_light_rect,
            order: 140,
            color: world.insert(Val::new(Vec4::new(1.0, 1.0, 1.0, 1.0))),
            radius: THUMB_RADIUS + 1.0,
            width: 0.0,
        });

        world.insert(RRect {
            common: self.common,
            rect: thumb_shadow_rect,
            order: 130,
            color: world.insert(Val::new(Vec4::new(0.0, 0.0, 0.0, 1.0))),
            radius: THUMB_RADIUS + 2.0,
            width: 0.0,
        });

        world.insert(ToolRingCollider {
            common: self.common,
            target: this.untyped(),
            rect: self.rect,
            inner_radius: self.inner_radius,
            outer_radius: OUTER_RADIUS,
            order: 100,
        });

        world.observer(this, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;
            let delta = event.position - I64Vec2::q32_from_i32(rect.origin);
            let uv = (delta.q32_as_f64() / rect.extend.as_dvec2()).as_vec2();
            let delta = uv - Vec2::splat(0.5);

            this.hsl.x = (delta.y.atan2(delta.x) / TAU).rem_euclid(1.0);
            world.queue_trigger(this.handle(), ColorHsla(this.hsl));
        });

        world.observer(this, move |&SetColorHsla(hsl), world| {
            let mut this = world.fetch_mut(this);
            let rect = world.fetch(this.rect).val;
            let mut thumb_rect = world.fetch_mut(thumb_rect);
            let mut thumb_color_val = world.fetch_mut(thumb_color_val);
            this.hsl = hsl;
            thumb_rect.val = square_thumb(rect, this.hsl);
            thumb_color_val.val = thumb_color(hsl);
            world.queue_trigger(thumb_rect.handle(), ValChanged);
            world.queue_trigger(thumb_color_val.handle(), ValChanged);
            world.queue_trigger(quad, SetQuadMaterial(this.mat()));
        });
    }

    fn mat(&self) -> HslRingMaterial {
        HslRingMaterial {
            hsl: self.hsl,
            inner_radius: self.inner_radius,
            _pad: [0; 3],
        }
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
