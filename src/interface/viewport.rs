use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

pub struct InterfaceViewport {
    width: u32,
    height: u32,

    camera: [i32; 2],

    zoom: f32,

    buffer: Buffer,
}
impl InterfaceViewport {
    pub fn new(
        device: &Device,
        width: u32,
        height: u32,
        camera: [i32; 2],
        zoom: f32,
    ) -> InterfaceViewport {
        let viewport_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("viewport_buffer"),
            contents: bytemuck::bytes_of(&InterfaceViewportBind {
                width,
                height,
                camera,
                zoom,
                _padding: 0
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        InterfaceViewport {
            width,
            height,
            camera,
            zoom,
            buffer: viewport_buffer,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, queue: &Queue) {
        self.width = width;
        self.height = height;
        queue.write_buffer(
            &self.buffer,
            InterfaceViewportBind::OFFSET_WIDTH_HEIGHT,
            bytemuck::bytes_of(&[width as i32, height as i32]),
        );
    }

    pub fn get_camera(&self) -> [i32; 2] {
        self.camera
    }

    pub fn set_camera(&mut self, position: [i32; 2], queue: &Queue) {
        self.camera = position;
        queue.write_buffer(
            &self.buffer,
            InterfaceViewportBind::OFFSET_CAMERA,
            bytemuck::bytes_of(&position),
        );
    }

    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    pub fn set_zoom(&mut self, zoom: f32, queue: &Queue) {
        self.zoom = zoom;
        queue.write_buffer(
            &self.buffer,
            InterfaceViewportBind::OFFSET_ZOOM,
            bytemuck::bytes_of(&zoom),
        );
    }

    pub fn world_to_screen(&self, point: [i32; 2]) -> [f64; 2] {
        let x = (point[0] - self.camera[0]) as f64 / self.width as f64 * 2.0;
        let y = (point[1] - self.camera[1]) as f64 / self.height as f64 * 2.0;
        [x * self.zoom as f64, y * self.zoom as f64]
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> [i32; 2] {
        let x = (point[0] / self.zoom as f64 * self.width as f64 / 2.0) as i32 + self.camera[0];
        let y = (point[1] / self.zoom as f64 * self.height as f64 / 2.0) as i32 + self.camera[1];
        [x, y]
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
    zoom: f32,
    /// 8 bytes alignment in WGSL
    _padding: u32,
}
impl InterfaceViewportBind {
    const OFFSET_WIDTH_HEIGHT: BufferAddress = 0;
    const OFFSET_CAMERA: BufferAddress = size_of::<[i32; 2]>() as BufferAddress;
    const OFFSET_ZOOM: BufferAddress = size_of::<[i32; 4]>() as BufferAddress;
}
