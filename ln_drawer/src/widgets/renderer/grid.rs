use ln_world::{Element, Handle, World};
use wgpu::ShaderSource;

use crate::{
    measures::Rectangle,
    render::rectangle::{RectangleMeshDescriptor, RectangleMeshMaterial},
    widgets::WidgetDestroyed,
};

pub struct Grid;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GridMaterial(u32);

impl Grid {
    fn attach_render(&mut self, world: &World, this: Handle<Self>) {
        let rectangle = world.build(RectangleMeshDescriptor {
            rect: Rectangle::default(),
            visible: true,
            order: -1000,
            material: GridMaterial(1024),
        });

        world.observer(this, move |&WidgetDestroyed, world| {
            world.remove(rectangle).unwrap();
        });

        world.dependency(rectangle, this);
    }
}

impl RectangleMeshMaterial for GridMaterial {
    fn label() -> &'static str {
        "grid"
    }

    fn shader() -> ShaderSource<'static> {
        ShaderSource::Wgsl(
            format!(
                "{}{}",
                include_str!("lib_camera.wgsl"),
                include_str!("grid.wgsl")
            )
            .into(),
        )
    }

    fn vertex() -> Option<Option<&'static str>> {
        Some(Some("vs_main"))
    }

    fn fragment() -> Option<&'static str> {
        Some("fs_main")
    }
}

impl Element for Grid {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_render(world, this);
    }
}
