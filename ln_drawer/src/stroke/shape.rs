use wgpu::{
    BindGroupLayout, ComputePipeline, ComputePipelineDescriptor, PipelineLayoutDescriptor,
    ShaderModuleDescriptor, ShaderSource,
};

use crate::render::Render;

pub struct RoundBrush {
    pub pipeline: ComputePipeline,
}

impl RoundBrush {
    pub fn new(render: &Render, chunk: &BindGroupLayout, draw: &BindGroupLayout) -> Self {
        let device = &render.device;

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("round_brush"),
            bind_group_layouts: &[chunk, draw],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("round_brush"),
            source: ShaderSource::Wgsl(include_str!("round.wgsl").into()),
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("round_brush"),
            layout: Some(&pipeline),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        RoundBrush { pipeline }
    }
}
