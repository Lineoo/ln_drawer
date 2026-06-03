use ln_world::{Element, Handle, World};
use wgpu::ShaderSource;

use crate::{
    measures::Rectangle,
    render::rectangle::{RectangleMeshDescriptor, RectangleMeshMaterial},
    widgets::WidgetDestroyed,
};

/// Standard palette for picking hsl color. Contains a circle of hue value and a square
/// whose x axis stands for saturation and y axis stands for lightness.
///
/// Corresponding material is [`PaletteHslMaterial`].
///
/// Possible events are [`WidgetRectangle`], [`WidgetHsla`] and [`WidgetDestroyed`].
pub struct Grid;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GridMaterial(u64);

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

    fn vertex() -> Option<ShaderSource<'static>> {
        Some(ShaderSource::Wgsl(include_str!("grid.wgsl").into()))
    }

    fn fragment() -> ShaderSource<'static> {
        ShaderSource::Wgsl(include_str!("grid.wgsl").into())
    }

    fn fragment_entry_point() -> Option<&'static str> {
        Some("main")
    }
}

impl Element for Grid {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.attach_render(world, this);
    }
}
