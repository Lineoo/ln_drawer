use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::lnwin::Viewport;

pub struct InterfaceViewport {
    buffer: Buffer,
}
impl InterfaceViewport {
    pub fn new(
        viewport: &Viewport,
        device: &Device,
    ) -> InterfaceViewport {
        let viewport_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("viewport_buffer"),
            contents: bytemuck::bytes_of(&InterfaceViewportBind {
                width: viewport.width,
                height: viewport.height,
                camera: viewport.camera,
                zoom: viewport.zoom,
                _padding: 0,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        InterfaceViewport {
            buffer: viewport_buffer,
        }
    }

    pub fn resize(&self, viewport: &Viewport, queue: &Queue) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::bytes_of(&InterfaceViewportBind {
                width: viewport.width,
                height: viewport.height,
                camera: viewport.camera,
                zoom: viewport.zoom,
                _padding: 0,
            }),
        );
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InterfaceViewportBind {
    width: u32,
    height: u32,
    camera: [i32; 2],
    zoom: i32,
    /// 8 bytes alignment in WGSL
    _padding: u32,
}