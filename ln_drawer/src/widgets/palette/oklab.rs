use std::f32::consts::TAU;

use glam::{I64Vec2, Vec3A};
use ln_world::{Element, Handle, World};
use palette::Oklab;

use crate::{
    measures::{FI64Ext, Rectangle},
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        palette::utils::find_gamut_intersection,
        renderer::quad::{QuadMaterial, QuadMeshDescriptor},
        shaders::{LIB_COLORSPACE, LIB_CONSTANT},
    },
};

const BAND_WIDTH: f32 = 0.1;

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteOkLchMaterial`].
pub struct PaletteOklab {
    pub rect: Rectangle,
    pub color: Oklab,
    pub enabled: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteOklabMaterial {
    oklab: Vec3A,
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
    _pad: u32,
}

pub struct PaletteColorOklab(pub Oklab);

impl PaletteOklab {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let rectangle = world.build(QuadMeshDescriptor {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: PaletteOklabMaterial {
                oklab: Vec3A::new(self.color.l, self.color.a, self.color.b),
                band_width: BAND_WIDTH,
                main_knob_size: 0.015,
                hue_knob_size: 0.005,
                _pad: 0,
            },
        });

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 100,
            enabled: self.enabled,
        });

        world.dependency(collider, this);

        let mut lock = 0;
        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);

            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();
            let size = (0.5 - BAND_WIDTH) * 2f32.sqrt();
            let suv = (uv - 0.5) / size + 0.5;

            let delta = uv - 0.5;
            let radius = delta.length();
            let angle = f32::atan2(delta.y, delta.x);

            if lock == 1 || (lock == 0 && suv.x > 0. && suv.x < 1. && suv.y > 0. && suv.y < 1.) {
                lock = 1;
                this.color.a = ((suv.x - 0.5) * 0.8).clamp(-0.4, 0.4);
                this.color.b = ((suv.y - 0.5) * 0.8).clamp(-0.4, 0.4);
                this.color = clip_to_srgb_gamut(this.color);
                world.queue_trigger(this.handle(), PaletteColorOklab(this.color));
            } else if lock == 2 || (lock == 0 && radius > 0.5 - BAND_WIDTH && radius < 0.5) {
                lock = 2;
                this.color.l = angle.rem_euclid(TAU) / TAU;
                world.queue_trigger(this.handle(), PaletteColorOklab(this.color));
            } else {
                lock = 3;
            }

            if let PointerHitStatus::Release = event.status {
                lock = 0;
            }
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            rectangle.desc.visible = enabled;
            collider.enabled = enabled;
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            rectangle.desc.rect = rect;
            collider.rect = rect;
        });

        world.observer(this, move |&PaletteColorOklab(color), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.material.oklab = Vec3A::new(color.l, color.a, color.b);
        });

        world.dependency(rectangle, this);
    }
}

/// Clips an Oklch color into the sRGB gamut while keeping lightness and hue
/// constant, only compressing chroma.
///
/// Ported from Björn Ottosson's "sRGB gamut clipping".
/// https://bottosson.github.io/posts/gamutclipping/
fn clip_to_srgb_gamut(color: Oklab) -> Oklab {
    let l = color.l.clamp(0.0, 1.0);
    let (b, a) = (color.b, color.a);
    let c = (a * a + b * b).sqrt().max(1e-5);
    let t = find_gamut_intersection(a / c, b / c, l, c, l).min(1.0);
    Oklab::new(l, t * a, t * b)
}

impl QuadMaterial for PaletteOklabMaterial {
    fn label() -> &'static str {
        "palette_oklab"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_CONSTANT,
                include_str!("oklab.wgsl")
            )
            .into(),
        )
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for PaletteOklab {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
