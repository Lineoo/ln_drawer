use std::mem::size_of;

use bytemuck::{bytes_of, cast_slice};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
    ComputePass, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device,
    PipelineCompilationOptions, PipelineLayoutDescriptor, Queue, ShaderModuleDescriptor,
    ShaderSource, ShaderStages,
};

use crate::{
    measures::Rectangle,
    stroke::modifier::DrawProcessedStorage,
};

const WORKGROUP_SIZE: u32 = 16;
const MAX_STROKE: u64 = 200;

const LAYOUT_DISPATCH_DRAW: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_brush_dispatch_draw"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 2,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ],
};

pub struct Brush {
    device: Device,
    queue: Queue,

    brush_round: ComputePipeline,
    erase_round: ComputePipeline,

    dispatch: Buffer,
    draws_length: Buffer,
    draws_array: Buffer,

    dispatch_group_draw: BindGroup,
}

pub struct BrushConfig {
    pub device: Device,
    pub queue: Queue,
    pub chunk_draw_layout: BindGroupLayout,
}

impl Brush {
    pub fn new(config: BrushConfig) -> Self {
        let device = &config.device;

        let dispatch_draw_layout =
            device.create_bind_group_layout(&LAYOUT_DISPATCH_DRAW);

        let dispatch = device.create_buffer(&BufferDescriptor {
            label: Some("layer_brush_dispatch"),
            size: size_of::<u32>() as u64 * 8,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_length = device.create_buffer(&BufferDescriptor {
            label: Some("layer_brush_draws_length"),
            size: size_of::<u32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_array = device.create_buffer(&BufferDescriptor {
            label: Some("layer_brush_draws_array"),
            size: size_of::<DrawProcessedStorage>() as u64 * MAX_STROKE,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dispatch_group_draw = device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_brush_dispatch_draw"),
            layout: &dispatch_draw_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &dispatch,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &draws_length,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &draws_array,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let (brush_round, erase_round) =
            brush_pipelines(device, &dispatch_draw_layout, &config.chunk_draw_layout);

        Brush {
            device: config.device,
            queue: config.queue,
            brush_round,
            erase_round,
            dispatch,
            draws_length,
            draws_array,
            dispatch_group_draw,
        }
    }

    pub fn paint(
        &self,
        dirty: Rectangle,
        draws: &[DrawProcessedStorage],
        paint_chunks: &[(super::ChunkKey, &BindGroup)],
        erase: bool,
    ) {
        super::write_dispatch_uniform(&self.queue, &self.dispatch, dirty);
        upload_draws(&self.draws_length, &self.draws_array, draws, &self.queue);

        let mut encoder = self
            .device
            .create_command_encoder(&ENCODER_DESC);
        let mut cpass = encoder.begin_compute_pass(&CPASS_DESC);

        if erase {
            cpass.set_pipeline(&self.erase_round);
        } else {
            cpass.set_pipeline(&self.brush_round);
        }

        cpass.set_bind_group(0, Some(&self.dispatch_group_draw), &[]);
        for &(key, chunk_bind) in paint_chunks {
            cpass.set_bind_group(1, Some(chunk_bind), &[]);
            dispatch_workgroups(dirty, key, &mut cpass);
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }
}

// --- Pipelines ---

fn brush_pipelines(
    device: &Device,
    dispatch_draw_layout: &BindGroupLayout,
    chunk_draw_layout: &BindGroupLayout,
) -> (ComputePipeline, ComputePipeline) {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[dispatch_draw_layout, chunk_draw_layout],
        immediate_size: 0,
    });

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_brush"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("../stroke/lib_colorspace.wgsl"),
                include_str!("../stroke/lib_dispatch.wgsl"),
                include_str!("../stroke/round.wgsl"),
            )
            .into(),
        ),
    });

    let brush_round = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("layer_brush_round"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    });

    let erase_round = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("layer_erase_round"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_erase"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    });

    (brush_round, erase_round)
}

fn upload_draws(
    draws_length: &Buffer,
    draws_array: &Buffer,
    draws: &[DrawProcessedStorage],
    queue: &Queue,
) {
    queue.write_buffer(draws_length, 0, bytes_of(&(draws.len() as u32)));
    queue.write_buffer(draws_array, 0, cast_slice(draws));
}

// --- Dispatch ---

fn dispatch_workgroups(
    dirty: Rectangle,
    key: super::ChunkKey,
    cpass: &mut ComputePass,
) {
    let scale = 2u32.pow(key.2 as u32);
    cpass.dispatch_workgroups(
        dirty.extend.x.saturating_sub(1) / scale / WORKGROUP_SIZE + 1,
        dirty.extend.y.saturating_sub(1) / scale / WORKGROUP_SIZE + 1,
        1,
    );
}

const ENCODER_DESC: CommandEncoderDescriptor<'_> = CommandEncoderDescriptor {
    label: Some("layer_brush"),
};

const CPASS_DESC: ComputePassDescriptor<'_> = ComputePassDescriptor {
    label: Some("layer_brush"),
    timestamp_writes: None,
};
