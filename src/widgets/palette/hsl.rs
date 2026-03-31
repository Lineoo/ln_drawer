use palette::{FromColor, Hsl, Hsla, SetHue, Srgba};

use crate::{
    layout::{LayoutControl, LayoutRectangle, Layouts},
    measures::{Position, Rectangle, Size},
    render::{
        RenderControl,
        rectangle::{RectangleMesh, RectangleMeshDescriptor, RectangleMeshMaterial},
        wireframe::{Wireframe, WireframeDescriptor},
    },
    stroke::StrokeLayer,
    tools::{collider::ToolCollider, pointer::PointerHit, touch::MultiTouchData},
    widgets::{WidgetDestroyed, WidgetHsla, WidgetRectangle},
    world::{Descriptor, Element, Handle, World},
};

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteHslMaterial`].
///
/// Possible events are [`WidgetRectangle`], [`WidgetHsla`] and [`WidgetDestroyed`].
pub struct PaletteHsl {
    pub rect: Rectangle,
    pub color: Hsla,
    pub order: isize,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteHslMaterial {
    h: f32,
    s: f32,
    l: f32,
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
                h: self.color.hue.into_degrees() / 360.0,
                s: self.color.saturation,
                l: self.color.lightness,
            },
        });

        world.observer(this, move |&WidgetRectangle(rect), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.rect = rect;
        });

        world.observer(this, move |&WidgetHsla(hsla), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            rectangle.desc.material.h = hsla.hue.into_degrees() / 360.0;
        });

        world.observer(this, move |&WidgetDestroyed, world| {
            world.remove(rectangle).unwrap();
        });
    }

    fn attach_pointer(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: 60,
            enabled: true,
        });

        world.observer(collider, move |event: &PointerHit, world| {
            let mut this = world.fetch_mut(this).unwrap();
            let delta = event.position - this.rect.origin;
            let saturation = delta.x as f32 / this.rect.extend.w as f32;
            let lightness = delta.y as f32 / this.rect.extend.h as f32;
            let color = Hsla::new(this.color.hue, saturation, lightness, this.color.alpha);
            this.color = color;
            world.queue_trigger(this.handle(), WidgetHsla(color));
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
