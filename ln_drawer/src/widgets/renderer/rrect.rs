use glam::{UVec2, Vec4};
use ln_world::{Element, Handle, World};

use crate::{
    layout::val::{Val, ValChanged},
    measures::Rectangle,
    widgets::{
        common::WidgetCommon,
        renderer::quad::{QuadMaterial, QuadMesh, SetQuadMaterial},
        shaders::shader_compile,
    },
};

pub struct RRect {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
    pub order: isize,
    pub color: Handle<Val<Vec4>>,
    pub radius: f32,
    pub width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RRectMaterial {
    color: Vec4,
    radius: f32,
    width: f32,
    _pad: [u32; 2],
}

impl RRect {
    fn init(&self, world: &World, this: Handle<Self>) {
        world.scope("rrect");
        world.set_autodeps(Some(this));
        world.dependency(this, self.color);

        let color = world.fetch(self.color).val;
        let quad = world.insert(QuadMesh {
            common: self.common,
            rect: self.rect,
            order: self.order,
            material: RRectMaterial {
                color,
                radius: self.radius,
                width: self.width,
                _pad: [0; 2],
            },
        });

        world.observer(self.color, move |ValChanged, world| {
            let this = world.fetch(this);
            let color = world.fetch(this.color).val;
            world.queue_trigger(
                quad,
                SetQuadMaterial(RRectMaterial {
                    color,
                    radius: this.radius,
                    width: this.width,
                    _pad: [0; 2],
                }),
            );
        });
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
