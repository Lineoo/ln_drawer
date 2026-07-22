use std::{mem::size_of, sync::mpsc::Sender};

use bytemuck::{bytes_of, cast_slice};
use glam::UVec2;
use palette::Srgba;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Device, PipelineCompilationOptions,
    PipelineLayoutDescriptor, Queue, ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

use crate::{
    layer::{Layer, LayerConfig, stream::ThreadInput},
    measures::{FI64Ext, Rectangle},
    stroke::{
        dirty::Dirty,
        interpolate::{Draw, Interpolation},
        modifier::{DrawProcessedStorage, Modifier},
    },
};

const WORKGROUP_SIZE: u32 = 16;
const MAX_STROKE: u64 = 200;

const DEFAULT_INTERPOLATION: Interpolation = Interpolation {
    step: |draw| draw.size / 5.0,
};
const DEFAULT_MODIFIER: Modifier = Modifier {
    min_size: 0.5,
    max_size: 6.0,
    size_force_exp: 1.0,
    min_flow: 0.1,
    max_flow: 1.0,
    flow_force_exp: 2.0,
    softness: 0.2,
    color: Srgba::new(0.0, 0.0, 0.0, 1.0),
};
const DEFAULT_DIRTY: Dirty = Dirty {
    bounding: |draw| {
        Rectangle::new_half(
            draw.position.q32_round(),
            UVec2::splat((draw.size * 2.0).ceil() as u32),
        )
    },
};

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

    pub scratch: Layer,

    brush_round: ComputePipeline,
    erase_round: ComputePipeline,

    dispatch: Buffer,
    draws_length: Buffer,
    draws_array: Buffer,

    dispatch_group_draw: BindGroup,

    pub erase: bool,
    pub interpolation: Interpolation,
    pub modifier: Modifier,
    pub dirty: Dirty,

    prev: Option<Draw>,
    stroke: Option<Stroke>,
}

struct Stroke {
    dirty: Rectangle,
}

pub struct BrushConfig {
    pub device: Device,
    pub queue: Queue,
    pub chunk_draw_layout: BindGroupLayout,
    pub scratch: LayerConfig,
}

impl Brush {
    pub fn new(config: BrushConfig) -> Self {
        let device = &config.device;

        let dispatch_draw_layout = device.create_bind_group_layout(&LAYOUT_DISPATCH_DRAW);

        let (dispatch, draws_length, draws_array, dispatch_group_draw) =
            dispatch_group(device, &dispatch_draw_layout);

        let (brush_round, erase_round) =
            brush_pipelines(device, &dispatch_draw_layout, &config.chunk_draw_layout);

        let scratch = Layer::new(config.scratch);

        Brush {
            device: config.device,
            queue: config.queue,
            scratch,
            brush_round,
            erase_round,
            dispatch,
            draws_length,
            draws_array,
            dispatch_group_draw,
            erase: false,
            interpolation: DEFAULT_INTERPOLATION,
            modifier: DEFAULT_MODIFIER,
            dirty: DEFAULT_DIRTY,
            prev: None,
            stroke: None,
        }
    }

    /// CPU-end draw process
    pub fn paint(&mut self, next: Draw) {
        let mut draw_buf = Vec::new();
        let curr = self
            .interpolation
            .interpolate(self.prev, next, &self.modifier, &mut draw_buf);
        self.prev = Some(curr);

        let dirty = self.dirty.compute(curr.position.q32_round(), &draw_buf);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

        if let Some(stroke) = &mut self.stroke {
            stroke.dirty = stroke.dirty.grow(dirty);
        } else {
            self.stroke = Some(Stroke { dirty });
        }

        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        self.paint_dispatch(dirty, &draw_stg);
    }

    /// dispatch to GPU for the rest
    fn paint_dispatch(&mut self, dirty: Rectangle, draws: &[DrawProcessedStorage]) {
        // Brush always use independent layer as scratch
        self.scratch.prepare_chunks(dirty);

        super::write_dispatch_uniform(&self.queue, &self.dispatch, dirty);
        upload_draws(&self.draws_length, &self.draws_array, draws, &self.queue);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_brush"),
            });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_brush"),
            timestamp_writes: None,
        });

        if self.erase {
            cpass.set_pipeline(&self.erase_round);
        } else {
            cpass.set_pipeline(&self.brush_round);
        }

        cpass.set_bind_group(0, Some(&self.dispatch_group_draw), &[]);

        let (src, dst) = super::rect_to_chunks(dirty, 0, self.scratch.chunk_size);
        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                if let Some(chunk) = self.scratch.chunks.get(&(x, y, 0)) {
                    cpass.set_bind_group(1, Some(&chunk.draw), &[]);
                    dispatch_workgroups(dirty, (x, y, 0), &mut cpass);
                }
            }
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn request_stream(&mut self, main: &mut Layer, tx: &Sender<ThreadInput>) {
        let Some(stroke) = &self.stroke else {
            return;
        };

        for level in 0..main.mipmap_levels {
            let (src, dst) = super::rect_to_chunks(stroke.dirty, level, main.chunk_size);
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    tx.send(ThreadInput::RequestReal((x, y, level))).unwrap();
                }
            }
        }
    }

    /// All finished, merge to main layer
    pub fn submit(&mut self, main: &mut Layer) {
        self.prev = None;
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        // TODO may prepare more chunks than you need, we might need
        //      to record extra chunks information
        main.prepare_chunks(stroke.dirty);
        main.merge_from(&self.scratch, stroke.dirty);
        main.generate_mipmaps(stroke.dirty);

        for (_key, chunk) in self.scratch.chunks.drain() {
            self.scratch.pool.push(chunk);
        }
    }

    /// All finished, merge to main layer and notify stream thread unsaved chunks
    pub fn submit_stream(&mut self, main: &mut Layer, tx: &Sender<ThreadInput>) {
        self.prev = None;
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        main.merge_from(&self.scratch, stroke.dirty);
        main.generate_mipmaps(stroke.dirty);

        for (_key, chunk) in self.scratch.chunks.drain() {
            self.scratch.pool.push(chunk);
        }

        for level in 0..main.mipmap_levels {
            let (src, dst) = super::rect_to_chunks(stroke.dirty, level, main.chunk_size);
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    tx.send(ThreadInput::MarkUnsaved((x, y, level))).unwrap();
                }
            }
        }
    }
}

// --- Resources --- //

fn dispatch_group(
    device: &Device,
    dispatch_draw_layout: &BindGroupLayout,
) -> (Buffer, Buffer, Buffer, BindGroup) {
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
        layout: dispatch_draw_layout,
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
    (dispatch, draws_length, draws_array, dispatch_group_draw)
}

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

// --- Utils --- //

fn upload_draws(
    draws_length: &Buffer,
    draws_array: &Buffer,
    draws: &[DrawProcessedStorage],
    queue: &Queue,
) {
    queue.write_buffer(draws_length, 0, bytes_of(&(draws.len() as u32)));
    queue.write_buffer(draws_array, 0, cast_slice(draws));
}

fn dispatch_workgroups(dirty: Rectangle, key: super::ChunkKey, cpass: &mut ComputePass) {
    let scale = 2u32.pow(key.2 as u32);
    cpass.dispatch_workgroups(
        dirty.extend.x.saturating_sub(1) / scale / WORKGROUP_SIZE + 1,
        dirty.extend.y.saturating_sub(1) / scale / WORKGROUP_SIZE + 1,
        1,
    );
}
