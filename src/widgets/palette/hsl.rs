use glam::Vec2;
use palette::{Hsla, RgbHue};

use crate::{
    layout::LayoutControl,
    measures::Rectangle,
    render::rectangle::{RectangleMeshDescriptor, RectangleMeshMaterial},
    tools::{collider::ToolCollider, pointer::PointerHit},
    widgets::{WidgetDestroyed, WidgetHsla, WidgetRectangle},
    world::{Element, Handle, World},
};

const BAND_WIDTH: f32 = 0.1;

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteHslMaterial`].
///
/// Possible events are [`WidgetRectangle`], [`WidgetHsla`] and [`WidgetDestroyed`].
pub struct PaletteHsl {
    pub rect: Rectangle,
    pub color: Hsla,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteHslMaterial {
    band_width: f32,
    main_knob_size: f32,
    hue_knob_size: f32,
    hue: f32,
    saturation: f32,
    lightness: f32,
}

impl PaletteHsl {
    fn respond_layout(&mut self, world: &World, this: Handle<Self>) {
        world.insert(LayoutControl {
            rectangle: Some(Box::new(move |world, rect| {
                let mut this = world.fetch_mut(this).unwrap();
                this.rect = rect;
                world.queue_trigger(this.handle(), WidgetRectangle(rect));
                rect
            })),
        });
    }

    fn attach_luni(&mut self, world: &World, this: Handle<Self>) {
        let rectangle = world.build(RectangleMeshDescriptor {
            rect: self.rect,
            visible: true,
            order: 60,
            material: PaletteHslMaterial {
                band_width: BAND_WIDTH,
                main_knob_size: 0.015,
                hue_knob_size: 0.005,
                hue: self.color.hue.into_degrees() / 360.0,
                saturation: self.color.saturation,
                lightness: self.color.lightness,
            },
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.rect = rect;
        });

        world.observer(this, move |&WidgetHsla(hsla), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.material.hue = hsla.hue.into_positive_degrees() / 360.0;
            rectangle.desc.material.saturation = hsla.saturation;
            rectangle.desc.material.lightness = hsla.lightness;
        });

        world.observer(this, move |&WidgetDestroyed, world| {
            world.remove(rectangle).unwrap();
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 100,
            enabled: true,
        });

        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let delta = event.position - this.rect.origin;

            let u = delta.x as f32 / this.rect.extend.w as f32;
            let v = delta.y as f32 / this.rect.extend.h as f32;
            let uv = Vec2::new(u, v);
            let size = (0.5 - BAND_WIDTH) * 2f32.sqrt();
            let suv = (uv - 0.5) / size + 0.5;

            let delta = uv - 0.5;
            let radius = delta.length();
            let angle = f32::atan2(delta.y, delta.x);

            if suv.x > 0. && suv.x < 1. && suv.y > 0. && suv.y < 1. {
                this.color.saturation = suv.x;
                this.color.lightness = suv.y;
                world.queue_trigger(this.handle(), WidgetHsla(this.color));
            } else if radius > 0.5 - BAND_WIDTH && radius < 0.5 {
                this.color.hue = RgbHue::from_radians(angle);
                world.queue_trigger(this.handle(), WidgetHsla(this.color));
            }
        });
    }
}

impl RectangleMeshMaterial for PaletteHslMaterial {
    fn label() -> &'static str {
        "palette_hsl"
    }

    fn fragment() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(include_str!("hsl.wgsl").into())
    }

    fn entry_point() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for PaletteHsl {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_luni(world, this);
        self.attach_pointer(world, this);
        self.respond_layout(world, this);
    }
}
