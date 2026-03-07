use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages,
};

use crate::{render::Render, world::Element};

pub struct RoundBrush {
    pub brush: BindGroup,
    pub brush_data: Buffer,
}

pub struct RoundBrushPipeline {
    pub pipeline: ComputePipeline,
    pub brush: BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RoundBrushUniform {
    pub size: f32,
    pub softness: f32,
}

impl Element for RoundBrushPipeline {}

impl RoundBrushPipeline {
    pub fn new(render: &Render, canvas: &BindGroupLayout, draw: &BindGroupLayout) -> Self {
        let device = &render.device;

        let brush = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("round_brush"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("round_brush"),
            bind_group_layouts: &[canvas, draw, &brush],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("round_brush"),
            source: ShaderSource::Wgsl(include_str!("round_brush.wgsl").into()),
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("round_brush"),
            layout: Some(&pipeline),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        RoundBrushPipeline { pipeline, brush }
    }
}

impl RoundBrush {
    pub fn new(render: &Render, pipeline: &RoundBrushPipeline) -> Self {
        let device = &render.device;

        let brush_data = device.create_buffer(&BufferDescriptor {
            label: Some("round_brush"),
            size: size_of::<RoundBrushUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let brush = device.create_bind_group(&BindGroupDescriptor {
            label: Some("round_brush"),
            layout: &pipeline.brush,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &brush_data,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        RoundBrush { brush, brush_data }
    }

    pub fn draw(
        &self,
        render: &Render,
        pipeline: &RoundBrushPipeline,
        canvas: &BindGroup,
        draw: &BindGroup,
    ) {
        let device = &render.device;

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("brush"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("brush"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&pipeline.pipeline);
        cpass.set_bind_group(0, Some(canvas), &[]);
        cpass.set_bind_group(1, Some(draw), &[]);
        cpass.set_bind_group(2, Some(&self.brush), &[]);
        cpass.dispatch_workgroups(4, 4, 1);

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);
    }
}
