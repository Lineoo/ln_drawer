pub mod blur;
pub mod round;
pub mod param;

use std::{mem::size_of, sync::mpsc::Sender};

use bytemuck::{bytes_of, cast_slice};
use glam::{I64Vec2, UVec2};
use hashbrown::HashMap;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Device, PipelineCompilationOptions,
    PipelineLayoutDescriptor, RenderPass, ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

use crate::{
    layer::{
        Chunk, ChunkLayout, ChunkPool, Layer, LayerPipeline, brush::round::RoundBrush,
        chunk_to_rect, create_chunk, create_chunk_texture, dispatch_workgroups,
        stream::ThreadInput, write_dispatch,
    },
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    widgets::shaders::LIB_CONSTANT,
};

pub const MAX_STROKE: u64 = 200;
const DRAWS_ARRAY_CAPACITY: u64 = 48 * 200;
const TEMP_CHUNK_SIZE: u32 = 1024;

pub struct LayerDrawPipeline {
    pub layer: LayerPipeline,

    // TODO currently unintendedly public for undo/redo system
    pub scratch_dst: Layer,
    scratch_swp: Layer,
    scratch_pool: ChunkPool,

    #[expect(unused)]
    bridge_dst: Chunk,

    pipelines: BrushPipelines,

    draws_dispatch: Buffer,
    draws_dispatch_group: BindGroup,
    draws_length: Buffer,
    draws_array: Buffer,

    prev: Option<Draw>,
    stroke: Option<Stroke>,
}

struct BrushPipelines {
    blur: ComputePipeline,
    round_over: ComputePipeline,
    round_erase: ComputePipeline,
}

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: I64Vec2,
    pub force: f32,
}

struct Stroke {
    dirty: Rectangle,
    chunks: Vec<super::ChunkKey>,
    replace: bool,
}

impl LayerDrawPipeline {
    pub fn new(layer: LayerPipeline) -> Self {
        let draw_dispatch_layout = layer.device.create_bind_group_layout(&LAYOUT_DRAW_DISPATCH);

        let (draws_dispatch, draws_length, draws_array, draws_dispatch_group) =
            draws_dispatch_group(&layer.device, &draw_dispatch_layout);

        let pipelines = brush_pipelines(&layer.device, &draw_dispatch_layout, &layer.chunk_layout);

        let scratch_dst = Layer {
            chunks: HashMap::new(),
            chunk_size: 512,
            controlled: false,
            mipmap_levels: 1,
        };

        let scratch_swp = Layer {
            chunks: HashMap::new(),
            chunk_size: 512,
            controlled: false,
            mipmap_levels: 1,
        };

        let bridge_texture = create_chunk_texture(&layer.device, TEMP_CHUNK_SIZE);
        let bridge_dst = create_chunk(
            &layer.device,
            &layer.chunk_layout,
            bridge_texture,
            chunk_to_rect((0, 0, 0), TEMP_CHUNK_SIZE),
        );

        LayerDrawPipeline {
            layer,
            scratch_dst,
            scratch_swp,
            scratch_pool: ChunkPool {
                list: Vec::new(),
                chunk_size: 512,
            },
            pipelines,
            bridge_dst,
            draws_dispatch,
            draws_dispatch_group,
            draws_length,
            draws_array,
            prev: None,
            stroke: None,
        }
    }

    /// CPU-end draw process
    pub fn draw(&mut self, dst: &Layer, brush: &RoundBrush, target: Draw) {
        let mut draws = Vec::new();
        let next = brush.interpolate(self.prev, target, &mut draws);

        let mut dirty = Rectangle::new_half(next.position.q32_as_i32(), UVec2::ZERO);
        for &draw in &draws {
            dirty = dirty.grow(draw.dirty());
        }

        self.prev = Some(next);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

        if let Some(stroke) = &mut self.stroke {
            stroke.dirty = stroke.dirty.grow(dirty);
        } else {
            self.stroke = Some(Stroke {
                dirty,
                replace: brush.replace_mode(),
                chunks: vec![],
            });
        }

        let mut draws_proc = Vec::with_capacity(draws.len());
        for draw in draws {
            draws_proc.push(draw.into_storage());
        }

        let reference_layer = match brush.replace_mode() {
            true => Some(dst),
            false => None,
        };

        // Brush always use uncontrolled layer as scratch
        self.layer.prepare_chunks(
            &mut self.scratch_dst,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
        );
        self.layer.prepare_chunks(
            &mut self.scratch_swp,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
        );

        let queue = &self.layer.queue;
        write_dispatch(&self.layer.queue, &self.draws_dispatch, dirty);
        write_dispatch(&self.layer.queue, &self.layer.dispatch, dirty);

        let draws_length = draws_proc.len() as u32;
        queue.write_buffer(&self.draws_length, 0, bytes_of(&draws_length));
        queue.write_buffer(&self.draws_array, 0, cast_slice(&draws_proc));

        let mut encoder = self
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_brush"),
            });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_brush"),
            timestamp_writes: None,
        });

        let (start, end) = super::rect_to_chunks(dirty, 0, self.scratch_dst.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0);
                if let Some(dst_chunk) = self.scratch_dst.chunks.get_mut(&key)
                    && let Some(swp_chunk) = self.scratch_swp.chunks.get_mut(&key)
                {
                    if let Some(stroke) = &mut self.stroke {
                        if !stroke.chunks.contains(&key) {
                            stroke.chunks.push(key);
                        }
                    }

                    match brush.erase {
                        true => cpass.set_pipeline(&self.pipelines.round_erase),
                        false => cpass.set_pipeline(&self.pipelines.round_over),
                    }

                    cpass.set_bind_group(0, Some(&self.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.write), &[]);
                    dispatch_workgroups(&mut cpass, dirty.extend);

                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
                    dispatch_workgroups(&mut cpass, dirty.extend);
                }
            }
        }

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);
    }

    pub fn scratch_render(&self, rpass: &mut RenderPass, camera: &Camera, debug: bool) {
        if let Some(stroke) = &self.stroke {
            self.layer
                .render(&self.scratch_dst, rpass, camera, debug, stroke.replace);
        }
    }

    pub fn request_stream(&mut self, dst: &Layer, tx: &Sender<ThreadInput>) {
        let Some(stroke) = &self.stroke else {
            return;
        };

        for level in 0..dst.mipmap_levels {
            let (start, end) = super::rect_to_chunks(stroke.dirty, level, dst.chunk_size);

            for x in start.0..end.0 {
                for y in start.1..end.1 {
                    let key = (x, y, level);

                    // When any lower chunks are loaded, we then request upper chunks
                    if level > 0
                        && super::lower_chunk_of(key)
                            .iter()
                            .all(|x| !dst.chunks.contains_key(x))
                    {
                        continue;
                    }

                    tx.send(ThreadInput::RequestReal(key)).unwrap();
                }
            }
        }
    }

    /// All finished, merge to dst layer and optionally notify stream thread unsaved chunks
    pub fn submit(&mut self, dst: &mut Layer, tx: Option<&Sender<ThreadInput>>) {
        self.prev = None;
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        // if failed to merge, we simply drop it.
        if tx.is_some() && !self.layer.validate_chunks(dst, stroke.dirty) {
            self.recycle_scratch();
            return;
        }

        debug_assert_eq!(dst.chunk_size, self.scratch_dst.chunk_size);
        debug_assert_eq!(self.scratch_swp.chunk_size, self.scratch_dst.chunk_size);

        let mut encoder = self
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_submit"),
            });
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_submit"),
            timestamp_writes: None,
        });

        write_dispatch(&self.layer.queue, &self.layer.dispatch, stroke.dirty);

        match stroke.replace {
            true => cpass.set_pipeline(&self.layer.merge_pipelines.replace.swap),
            false => cpass.set_pipeline(&self.layer.merge_pipelines.over.swap),
        };

        for (src_key, src_chunk) in &self.scratch_dst.chunks {
            if let Some(dst_chunk) = dst.chunks.get(src_key)
                && let Some(swp_chunk) = self.scratch_swp.chunks.get(src_key)
            {
                cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[]);
                cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                cpass.set_bind_group(3, Some(&swp_chunk.write), &[]);
                dispatch_workgroups(&mut cpass, UVec2::splat(self.scratch_swp.chunk_size));
            }
        }

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);

        // flip out dst layer's chunks
        // TODO instead of recycle these chunks, they should be consumed by undo/redo systems
        for (key, chunk) in self.scratch_swp.chunks.drain() {
            if let Some(tx) = tx {
                tx.send(ThreadInput::SwapChunk(key, chunk.clone())).unwrap();
            }
            let old = dst.chunks.insert(key, chunk);
            if let Some(old) = old {
                self.scratch_pool.list.push(old);
            }
        }

        self.layer.generate_mipmaps(dst, stroke.dirty);

        if let Some(tx) = tx {
            for level in 0..dst.mipmap_levels {
                let (start, end) = super::rect_to_chunks(stroke.dirty, level, dst.chunk_size);
                for x in start.0..end.0 {
                    for y in start.1..end.1 {
                        tx.send(ThreadInput::MarkUnsaved((x, y, level))).unwrap();
                    }
                }
            }
        }

        self.recycle_scratch();
    }

    fn recycle_scratch(&mut self) {
        for (_key, chunk) in self.scratch_dst.chunks.drain() {
            self.scratch_pool.list.push(chunk);
        }
        for (_key, chunk) in self.scratch_swp.chunks.drain() {
            self.scratch_pool.list.push(chunk);
        }
    }
}

// --- Resources --- //

const LAYOUT_DRAW_DISPATCH: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
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

fn draws_dispatch_group(
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
        size: DRAWS_ARRAY_CAPACITY,
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
    chunk_layout: &ChunkLayout,
) -> BrushPipelines {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[
            Some(dispatch_draw_layout),
            Some(&chunk_layout.read),
            Some(&chunk_layout.write),
        ],
        immediate_size: 0,
    });

    let round_pipeline = |label, formula| {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(
                format!(
                    "{}fn composite(src: vec4f, dst: vec4f) -> vec4f {{ return {}; }}",
                    include_str!("brush/round.wgsl"),
                    formula
                )
                .into(),
            ),
        });
        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        })
    };

    let blur_pipeline = |label| {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(
                format!("{}{}", LIB_CONSTANT, include_str!("brush/blur.wgsl"),).into(),
            ),
        });
        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        })
    };

    BrushPipelines {
        blur: blur_pipeline("blur"),
        round_over: round_pipeline("over", "src + dst * (1 - src.a)"),
        round_erase: round_pipeline("erase", "dst * (1 - src.a)"),
    }
}
