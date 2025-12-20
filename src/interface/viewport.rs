use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::lnwin::Viewport;

pub struct InterfaceViewport {
    pub layout: BindGroupLayout,
    pub bind: BindGroup,
    pub buffer: Buffer,
}

impl InterfaceViewport {
    pub fn new(viewport: &Viewport, device: &Device) -> InterfaceViewport {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("viewport"),
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

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("viewport"),
            contents: bytemuck::bytes_of(&ViewportUniform {
                size: viewport.size.into_array(),
                center: viewport.center.into_array(),
                center_fract: viewport.center.into_arrayf(),
                zoom: viewport.zoom.n,
                zoom_fract: viewport.zoom.nf,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("viewport"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        InterfaceViewport {
            layout,
            buffer,
            bind,
        }
    }

    pub fn resize(&self, viewport: &Viewport, queue: &Queue) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::bytes_of(&ViewportUniform {
                size: viewport.size.into_array(),
                center: viewport.center.into_array(),
                center_fract: viewport.center.into_arrayf(),
                zoom: viewport.zoom.n,
                zoom_fract: viewport.zoom.nf,
            }),
        );
    }
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
