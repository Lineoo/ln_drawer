use wgpu::{
    BindGroupLayout, ComputePipeline, ComputePipelineDescriptor, PipelineLayoutDescriptor,
    ShaderModuleDescriptor, ShaderSource,
};

use crate::render::Render;

pub struct RoundBrush {
    pub pipeline: ComputePipeline,
}

impl RoundBrush {
    pub fn new(render: &Render, dispatch: &BindGroupLayout, chunk: &BindGroupLayout) -> Self {
        let device = &render.device;

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("round_brush"),
            bind_group_layouts: &[dispatch, chunk],
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

pub struct PixelBrush {
    pub pipeline: ComputePipeline,
}

impl PixelBrush {
    pub fn new(render: &Render, dispatch: &BindGroupLayout, chunk: &BindGroupLayout) -> Self {
        let device = &render.device;

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("pixel_brush"),
            bind_group_layouts: &[dispatch, chunk],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("pixel_brush"),
            source: ShaderSource::Wgsl(include_str!("pixel.wgsl").into()),
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("pixel_brush"),
            layout: Some(&pipeline),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        PixelBrush { pipeline }
    }
}
