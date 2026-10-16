use std::f32::consts::TAU;

use glam::{I64Vec2, Vec4};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    tools::{
        collider::ToolRectCollider,
        pointer::{PointerHit, PointerHitStatus},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        common::WidgetCommon,
        renderer::quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
        shaders::shader_compile,
    },
};

const BAND_WIDTH: f32 = 0.1;

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteHslMaterial`].
pub struct HslPanel {
    pub rect: Rectangle,
    pub hsl: Vec4,
    pub enabled: bool,
    pub camera: Handle<Camera>,
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

pub struct ColorHsla(pub Vec4);
pub struct SetColorHsla(pub Vec4);

impl HslPanel {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            camera: self.camera,
            material: HslPanelMaterial {
                band_width: BAND_WIDTH,
                main_knob_size: 0.015,
                hue_knob_size: 0.005,
                hue: self.hsl.x,
                saturation: self.hsl.y,
                lightness: self.hsl.z,
            },
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

        let mut lock = 0;
        world.observer(this, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this);
            let delta = event.position - I64Vec2::q32_from_i32(this.rect.origin);

            let uv = (delta.q32_as_f64() / this.rect.extend.as_dvec2()).as_vec2();
            let size = (0.5 - BAND_WIDTH) * 2f32.sqrt();
            let suv = (uv - 0.5) / size + 0.5;

            let delta = uv - 0.5;
            let radius = delta.length();
            let angle = f32::atan2(delta.y, delta.x);

            if lock == 1 || (lock == 0 && suv.x > 0. && suv.x < 1. && suv.y > 0. && suv.y < 1.) {
                lock = 1;
                this.hsl.y = (suv.x).clamp(0.0, 1.0);
                this.hsl.z = (suv.y).clamp(0.0, 1.0);
                world.queue_trigger(this.handle(), ColorHsla(this.hsl));
            } else if lock == 2 || (lock == 0 && radius > 0.5 - BAND_WIDTH && radius < 0.5) {
                lock = 2;
                this.hsl.x = angle / TAU;
                world.queue_trigger(this.handle(), ColorHsla(this.hsl));
            } else {
                lock = 3;
            }

            if let PointerHitStatus::Release = event.status {
                lock = 0;
            }
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this);
            this.enabled = enabled;
            world.queue_trigger(collider, SetWidgetVisible(enabled));
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            this.rect = rect;
            world.queue_trigger(collider, SetWidgetRectangle(rect));
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetColorHsla(hsl), world| {
            let mut this = world.fetch_mut(this);
            let quad = world.fetch(quad);
            this.hsl = hsl;
            world.queue_trigger(
                quad.handle(),
                SetQuadMaterial(HslPanelMaterial {
                    hue: hsl.x,
                    saturation: hsl.y,
                    lightness: hsl.z,
                    ..quad.material
                }),
            );
        });
    }
}

impl QuadMaterial for HslPanelMaterial {
    fn label() -> &'static str {
        "palette_hsl"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("hsl.wgsl"), &[]).into())
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
