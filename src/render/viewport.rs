use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::measures::{DeltaFract, Fract, PositionFract, Size};
use crate::render::Render;
use crate::world::{Commander, Descriptor, Element, Handle, World};

pub struct Viewport {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,
    instance: Handle<ViewportInstance>,
    cmd: Commander,
}

#[derive(Debug, Default)]
pub struct ViewportDescriptor {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,
}

pub struct ViewportManager {
    pub layout: BindGroupLayout,
}

pub struct ViewportManagerDescriptor;

pub struct ViewportInstance {
    pub bind: BindGroup,
    pub uniform: Buffer,
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

impl Element for Viewport {}
impl Element for ViewportManager {}
impl Element for ViewportInstance {}

impl Descriptor for ViewportManagerDescriptor {
    type Target = ViewportManager;

    fn build(self, world: &World) -> Self::Target {
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

        ViewportManager { layout }
    }
}

impl Descriptor for ViewportDescriptor {
    type Target = Viewport;

    fn build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let manager = world.single_fetch::<ViewportManager>().unwrap();

        // instance //

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
            layout: &manager.layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let instance = world.insert(ViewportInstance { uniform, bind });

        Viewport {
            size: self.size,
            center: self.center,
            zoom: self.zoom,
            instance,
            cmd: world.commander(),
        }
    }
}

impl Viewport {
    pub fn screen_to_world_absolute(&self, point: [f64; 2]) -> PositionFract {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> DeltaFract {
        let scale = (self.zoom.n as f64 + self.zoom.nf as f64 * (-32f64).exp2()).exp2();
        let x = delta[0] / scale * self.size.w as f64 / 2.0;
        let y = delta[1] / scale * self.size.h as f64 / 2.0;
        DeltaFract::new(
            x.floor() as i32,
            (((x - x.floor()) * 32f64.exp2()).floor()) as u32,
            y.floor() as i32,
            (((y - y.floor()) * 32f64.exp2()).floor()) as u32,
        )
    }

    pub fn upload(&self) {
        let instance = self.instance;
        let uniform = ViewportUniform {
            size: Size::new(self.size.w.max(1), self.size.h.max(1)).into_array(),
            center: self.center.into_array(),
            center_fract: self.center.into_arrayf(),
            zoom: self.zoom.n,
            zoom_fract: self.zoom.nf,
        };

        self.cmd.queue(move |world| {
            let instance = world.fetch(instance).unwrap();
            let render = world.single_fetch::<Render>().unwrap();
            let bytes = bytemuck::bytes_of(&uniform);
            render.queue.write_buffer(&instance.uniform, 0, bytes);
        });
    }
}

impl Drop for Viewport {
    fn drop(&mut self) {
        let instance = self.instance;
        self.cmd.queue(move |world| {
            world.remove(instance);
        });
    }
}
