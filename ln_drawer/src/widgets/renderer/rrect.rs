use glam::{UVec2, Vec4};
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    measures::Rectangle,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        renderer::quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
        shaders::shader_compile,
    },
};

pub struct RRect {
    pub rect: Rectangle,
    pub order: isize,
    pub color: Srgba,
    pub radius: f32,
    pub width: f32,
    pub enabled: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RRectMaterial {
    color: Vec4,
    radius: f32,
    width: f32,
    _pad: [u32; 2],
}

pub struct SetRRectColor(pub Srgba);

impl RRect {
    fn init(&mut self, world: &World, this: Handle<Self>) {
        let quad = world.insert(QuadMesh {
            rect: self.rect,
            visible: self.enabled,
            order: self.order,
            material: RRectMaterial {
                color: Vec4::from(self.color.into_linear().into_components()),
                radius: self.radius,
                width: self.width,
                _pad: [0; 2],
            },
        });

        world.observer(this, move |&SetWidgetVisible(enabled), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.enabled = enabled;
            world.queue_trigger(quad, SetWidgetVisible(enabled));
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;
            world.queue_trigger(quad, SetWidgetRectangle(rect));
        });

        world.observer(this, move |&SetRRectColor(color), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.color = color;
            world.queue_trigger(
                quad,
                SetQuadMaterial(RRectMaterial {
                    color: Vec4::from(color.into_linear().into_components()),
                    radius: this.radius,
                    width: this.width,
                    _pad: [0; 2],
                }),
            );
        });

        world.dependency(quad, this);
    }
}

impl QuadMaterial for RRectMaterial {
    fn label() -> &'static str {
        "rrect"
    }

    fn shader() -> wgpu::ShaderSource<'static> {
        wgpu::ShaderSource::Wgsl(shader_compile(include_str!("rrect.wgsl"), &[]).into())
    }

    fn fragment() -> Option<&'static str> {
        Some("main")
    }

    fn edge(&self) -> UVec2 {
        UVec2::splat(20)
    }
}

impl Element for RRect {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
