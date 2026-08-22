use glam::{I64Vec2, Vec3A};
use ln_world::{Element, Handle, World};
use palette::{IntoColor, OklabHue, Oklch, Srgb, convert::FromColorUnclamped};

use crate::{
    measures::{FI64Ext, Rectangle},
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        renderer::quad::{QuadMaterial, QuadMeshDescriptor},
        shaders::{LIB_COLORSPACE, LIB_CONSTANT},
    },
};

const BAND_WIDTH: f32 = 0.1;

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteOkLchMaterial`].
pub struct PaletteOklch {
    pub rect: Rectangle,
    pub color: Oklch,
    pub enabled: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteOklchMaterial {
    oklch: Vec3A,
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
    _pad: u32,
}

pub struct PaletteColorOklch(pub Oklch);

impl PaletteOklch {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let rectangle = world.build(QuadMeshDescriptor {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: PaletteOklchMaterial {
                oklch: Vec3A::new(
                    self.color.l,
                    self.color.chroma,
                    self.color.hue.into_positive_radians(),
                ),
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
                this.color.chroma = (suv.x).clamp(0.0, 1.0) * 0.4;
                this.color.l = (suv.y).clamp(0.0, 1.0);
                world.queue_trigger(this.handle(), PaletteColorOklch(this.color));
            } else if lock == 2 || (lock == 0 && radius > 0.5 - BAND_WIDTH && radius < 0.5) {
                lock = 2;
                this.color.hue = OklabHue::from_radians(angle);
                world.queue_trigger(this.handle(), PaletteColorOklch(this.color));
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

        world.observer(this, move |&PaletteColorOklch(color), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.material.oklch =
                Vec3A::new(color.l, color.chroma, color.hue.into_positive_radians());
        });

        world.dependency(rectangle, this);
    }
}

impl QuadMaterial for PaletteOklchMaterial {
    fn label() -> &'static str {
        "palette_oklch"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_CONSTANT,
                include_str!("oklch.wgsl")
            )
            .into(),
        )
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for PaletteOklch {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
