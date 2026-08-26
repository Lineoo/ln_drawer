use glam::{I64Vec2, Vec3A};
use ln_world::{Element, Handle, World};
use palette::Oklab;

use crate::{
    measures::{FI64Ext, Rectangle},
    tools::{collider::ToolCollider, pointer::PointerHit},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        palette::utils::find_gamut_intersection,
        renderer::quad::{QuadMaterial, QuadMeshDescriptor},
        shaders::{LIB_COLORSPACE, LIB_CONSTANT},
    },
};

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
        let rectangle = world.build(QuadMeshDescriptor {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: OklabPolarMaterial {
                oklab: Vec3A::new(self.color.l, self.color.a, self.color.b),
            },
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

        world.observer(this, move |&SetColorOklab(color), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            let mut this = world.fetch_mut(this).unwrap();
            this.color = color;
            rectangle.desc.material.oklab = Vec3A::new(color.l, color.a, color.b);
        });

        world.dependency(rectangle, this);
    }
}

impl OklabBar {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let rectangle = world.build(QuadMeshDescriptor {
            rect: self.rect,
            visible: self.enabled,
            order: 60,
            material: OklabBarMaterial {
                oklab: Vec3A::new(self.color.l, self.color.a, self.color.b),
            },
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

        world.observer(this, move |&SetColorOklab(color), world| {
            let mut rectangle = world.fetch_mut(rectangle).unwrap();
            let mut this = world.fetch_mut(this).unwrap();
            this.color = color;
            rectangle.desc.material.oklab = Vec3A::new(color.l, color.a, color.b);
        });

        world.dependency(rectangle, this);
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
        wgpu::ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_CONSTANT,
                include_str!("oklab_polar.wgsl")
            )
            .into(),
        )
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
        wgpu::ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_CONSTANT,
                include_str!("oklab_bar.wgsl")
            )
            .into(),
        )
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
