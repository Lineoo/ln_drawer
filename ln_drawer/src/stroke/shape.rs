use wgpu::{
    BindGroupLayout, ComputePipeline, ComputePipelineDescriptor, PipelineLayoutDescriptor,
    ShaderModuleDescriptor, ShaderSource,
};

use crate::render::Render;

pub fn brush_round(
    render: &Render,
    dispatch: &BindGroupLayout,
    chunk: &BindGroupLayout,
) -> ComputePipeline {
    let device = &render.device;

    let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("round_brush"),
        bind_group_layouts: &[dispatch, chunk],
        immediate_size: 0,
    });

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("round_brush"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("lib_colorspace.wgsl"),
                include_str!("lib_dispatch.wgsl"),
                include_str!("round.wgsl")
            )
            .into(),
        ),
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("round_brush"),
        layout: Some(&pipeline),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    })
}
