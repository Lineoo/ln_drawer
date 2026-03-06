use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBinding, BufferDescriptor,
    BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, StorageTextureAccess, Texture, TextureFormat, TextureViewDescriptor,
    TextureViewDimension,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{render::Render, world::Element};

pub struct RoundBrush {
    pipeline: ComputePipeline,
    canvas: BindGroupLayout,
    brush: BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RoundBrushBind {
    position: [f32; 2],
    size: f32,
    softness: f32,
}

impl Element for RoundBrush {}

impl RoundBrush {
    pub fn new(render: &Render) -> Self {
        let device = &render.device;

        let canvas = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canvas"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadWrite,
                    format: TextureFormat::Rgba8Unorm,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            }],
        });

        let brush = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("brush"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("brush"),
            bind_group_layouts: &[&canvas, &brush],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("brush"),
            source: ShaderSource::Wgsl(include_str!("round_brush.wgsl").into()),
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("brush"),
            layout: Some(&pipeline),
            module: &shader,
            entry_point: Some("round_brush"),
            compilation_options: Default::default(),
            cache: None,
        });

        RoundBrush {
            pipeline,
            canvas,
            brush,
        }
    }

    pub fn draw(
        &self,
        texture: &Texture,
        position: [f32; 2],
        size: f32,
        softness: f32,
        render: &Render,
    ) {
        let device = &render.device;

        let canvas = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas"),
            layout: &self.canvas,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(
                    &texture.create_view(&TextureViewDescriptor::default()),
                ),
            }],
        });

        let brush = device.create_bind_group(&BindGroupDescriptor {
            label: Some("brush"),
            layout: &self.brush,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("brush_buffer"),
                        contents: bytemuck::bytes_of(&RoundBrushBind {
                            position,
                            size,
                            softness,
                        }),
                        usage: BufferUsages::UNIFORM,
                    }),
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("brush"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("brush"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, Some(&canvas), &[]);
        cpass.set_bind_group(1, Some(&brush), &[]);
        cpass.dispatch_workgroups(4, 4, 1);

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);
    }
}
