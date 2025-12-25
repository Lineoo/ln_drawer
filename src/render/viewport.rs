use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::event::WindowEvent;

use crate::lnwin::Lnwindow;
use crate::measures::{Fract, PositionFract, Size};
use crate::render::Render;
use crate::world::{Descriptor, Element, Handle, World};

pub struct Viewport {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,

    pub bind: BindGroup,
    pub uniform: Buffer,
    pub layout: BindGroupLayout,

    queue: Queue,
}

#[derive(Debug, Default)]
pub struct ViewportDescriptor {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewportUniform {
    size: [u32; 2],
    center: [i32; 2],
    center_fract: [u32; 2],
    zoom: i32,
    zoom_fract: u32,
}

impl Element for Viewport {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world, lnwindow| {
            if let WindowEvent::Resized(size) = event {
                let mut viewport = world.fetch_mut(this).unwrap();
                let lnwindow = world.fetch(lnwindow).unwrap();

                viewport.size.w = size.width;
                viewport.size.h = size.height;

                viewport.upload();
            }
        });
    }
}

impl Descriptor for ViewportDescriptor {
    type Target = Handle<Viewport>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();

        let layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("viewport_bind_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("viewport_uniform"),
            contents: bytemuck::bytes_of(&ViewportUniform {
                size: self.size.into_array(),
                center: self.center.into_array(),
                center_fract: self.center.into_arrayf(),
                zoom: self.zoom.n,
                zoom_fract: self.zoom.nf,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("viewport_bind"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        world.insert(Viewport {
            size: self.size,
            center: self.center,
            zoom: self.zoom,
            uniform,
            bind,
            layout,
            queue: render.queue.clone(),
        })
    }
}

impl Viewport {
    #[inline]
    pub fn screen_to_world_absolute(&self, point: [f64; 2]) -> PositionFract {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> PositionFract {
        let scale = (self.zoom.n as f64 + self.zoom.nf as f64 * (-32f64).exp2()).exp2();
        let x = delta[0] / scale * self.size.w as f64 / 2.0;
        let y = delta[1] / scale * self.size.h as f64 / 2.0;
        PositionFract::new(Fract::from_f64(x), Fract::from_f64(y))
    }

    pub fn upload(&self) {
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&ViewportUniform {
                size: Size::new(self.size.w.max(1), self.size.h.max(1)).into_array(),
                center: self.center.into_array(),
                center_fract: self.center.into_arrayf(),
                zoom: self.zoom.n,
                zoom_fract: self.zoom.nf,
            }),
        );
    }
}
