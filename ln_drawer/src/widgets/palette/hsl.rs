use glam::I64Vec2;
use ln_world::{Element, Handle, World};
use palette::{Hsla, RgbHue};

use crate::{
    measures::{FI64Ext, Rectangle},
    tools::{
        collider::ToolCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        renderer::quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
        shaders::{LIB_COLORSPACE, LIB_CONSTANT},
    },
};

const BAND_WIDTH: f32 = 0.1;

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteHslMaterial`].
pub struct HslPanel {
    pub rect: Rectangle,
    pub color: Hsla,
    pub enabled: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HslPanelMaterial {
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
    hue: f32,
    saturation: f32,
    lightness: f32,
}

pub struct ColorHsla(pub Hsla);
pub struct SetColorHsla(pub Hsla);

impl HslPanel {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: HslPanelMaterial {
                band_width: BAND_WIDTH,
                main_knob_size: 0.015,
                hue_knob_size: 0.005,
                hue: self.color.hue.into_degrees() / 360.0,
                saturation: self.color.saturation,
                lightness: self.color.lightness,
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
                this.color.saturation = (suv.x).clamp(0.0, 1.0);
                this.color.lightness = (suv.y).clamp(0.0, 1.0);
                world.queue_trigger(this.handle(), ColorHsla(this.color));
            } else if lock == 2 || (lock == 0 && radius > 0.5 - BAND_WIDTH && radius < 0.5) {
                lock = 2;
                this.color.hue = RgbHue::from_radians(angle);
                world.queue_trigger(this.handle(), ColorHsla(this.color));
            } else {
                lock = 3;
            }

            if let PointerHitStatus::Release = event.status {
                lock = 0;
            }
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.enabled = enabled;
            collider.enabled = enabled;
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();
            this.rect = rect;
            collider.rect = rect;
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorHsla(color), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let quad = world.fetch(quad).unwrap();
            this.color = color;
            world.queue_trigger(
                quad.handle(),
                SetQuadMaterial(HslPanelMaterial {
                    hue: color.hue.into_positive_degrees() / 360.0,
                    saturation: color.saturation,
                    lightness: color.lightness,
                    ..quad.material
                }),
            );
        });

        world.dependency(quad, this);
    }
}

impl QuadMaterial for HslPanelMaterial {
    fn label() -> &'static str {
        "palette_hsl"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_CONSTANT,
                include_str!("hsl.wgsl")
            )
            .into(),
        )
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for HslPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
