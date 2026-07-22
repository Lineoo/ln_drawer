pub mod brush;
pub mod stream;
pub mod wrapper;

use std::mem::size_of;

use bytemuck::bytes_of;
use glam::UVec2;
use hashbrown::HashMap;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendComponent,
    BlendFactor, BlendOperation, BlendState, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    ComputePass, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device,
    Extent3d, FilterMode, FragmentState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::{MSAA_STATE, camera::Camera},
};

pub type ChunkKey = (i32, i32, u8);

pub const DEFAULT_MIPMAP_DISABLED: u32 = 1;
pub const DEFAULT_MIPMAP_ENABLED: u32 = 8;
pub const DEFAULT_CHUNK_SIZE: u32 = 512;

const WORKGROUP_SIZE: UVec2 = UVec2::new(16, 16);

pub struct LayerConfig {
    pub device: Device,
    pub queue: Queue,
    pub surface_format: TextureFormat,
    pub mipmap_levels: u8,
    pub chunk_size: u32,
    pub controlled: bool,
    pub camera_bind_layout: BindGroupLayout,
}

pub struct Layer {
    device: Device,
    queue: Queue,

    chunk_size: u32,
    mipmap_levels: u8,
    controlled: bool,

    chunks: HashMap<ChunkKey, Chunk>,
    pool: Vec<Chunk>,

    dispatch: Buffer,

    chunk_render_layout: BindGroupLayout,
    chunk_draw_layout: BindGroupLayout,

    dispatch_group: BindGroup,
    sampler_group_unfiltered: BindGroup,
    sampler_group_filtered: BindGroup,

    render_pipelines: RenderPipelines,
    merge_pipelines: MergePipelines,
    mipmap_pipeline: ComputePipeline,
    clear_pipeline: ComputePipeline,
}

pub struct Chunk {
    pub key: Buffer,
    pub texture: Texture,
    pub render: BindGroup,
    pub draw: BindGroup,
}

struct RenderPipelines {
    over: RenderPipeline,
    over_debug: RenderPipeline,
    erase: RenderPipeline,
}

struct MergePipelines {
    over: ComputePipeline,
    erase: ComputePipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    dispatch_coords: [i32; 2],
    dispatch_size: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    chunk: [i32; 3],
    _pad: u32,
}

impl Layer {
    pub fn new(config: LayerConfig) -> Self {
        assert!(config.mipmap_levels >= 1, "mipmap_levels must be >= 1");

        let device = &config.device;

        let chunk_render_layout = device.create_bind_group_layout(&LAYOUT_CHUNK_RENDER);
        let chunk_draw_layout = device.create_bind_group_layout(&LAYOUT_CHUNK_DRAW);
        let dispatch_layout = device.create_bind_group_layout(&LAYOUT_DISPATCH);
        let sampler_layout = device.create_bind_group_layout(&LAYOUT_SAMPLER);

        let (dispatch, dispatch_group) = dispatch_group(device, &dispatch_layout);

        let (sampler_group_unfiltered, sampler_group_filtered) =
            sampler_groups(device, &sampler_layout);

        let render_pipelines =
            render_pipelines(&config, device, sampler_layout, &chunk_render_layout);

        let merge_pipelines = merge_pipelines(device, &chunk_draw_layout, &dispatch_layout);
        let mipmap_pipeline = mipmap_pipeline(device, &chunk_draw_layout, &dispatch_layout);
        let clear_pipeline = clear_pipeline(device, &chunk_draw_layout);

        Layer {
            device: config.device,
            queue: config.queue,
            chunk_size: config.chunk_size,
            mipmap_levels: config.mipmap_levels,
            controlled: config.controlled,
            chunks: HashMap::new(),
            pool: Vec::new(),
            dispatch,
            chunk_render_layout,
            chunk_draw_layout,
            dispatch_group,
            sampler_group_unfiltered,
            sampler_group_filtered,
            render_pipelines,
            mipmap_pipeline,
            merge_pipelines,
            clear_pipeline,
        }
    }

    pub fn validate_chunks(&mut self, dirty: Rectangle) -> bool {
        for mipmap in 0..self.mipmap_levels {
            let (src, dst) = rect_to_chunks(dirty, mipmap, self.chunk_size);
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let key = (x, y, mipmap);
                    if !self.chunks.contains_key(&key) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    pub fn missing_chunks(&self, dirty: Rectangle) -> Vec<ChunkKey> {
        let mut missing = Vec::new();
        for mipmap in 0..self.mipmap_levels {
            let (src, dst) = rect_to_chunks(dirty, mipmap, self.chunk_size);
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let key = (x, y, mipmap);
                    if !self.chunks.contains_key(&key) {
                        missing.push(key);
                    }
                }
            }
        }
        missing
    }

    /// Assume `self.controlled` is false.
    pub fn prepare_chunks(&mut self, rect: Rectangle) {
        debug_assert!(!self.controlled, "controlled layer cannot prepare chunks");
        for mipmap in 0..self.mipmap_levels {
            let (src, dst) = rect_to_chunks(rect, mipmap, self.chunk_size);
            for chunk_x in src.0..dst.0 {
                for chunk_y in src.1..dst.1 {
                    let key = (chunk_x, chunk_y, mipmap);
                    if !self.chunks.contains_key(&key) {
                        let chunk = self.create_empty_chunk(key);
                        self.chunks.insert(key, chunk);
                    }
                }
            }
        }
    }

    pub fn generate_mipmaps(&mut self, dirty: Rectangle) {
        if self.mipmap_levels <= 1 {
            return;
        }

        write_dispatch_uniform(&self.queue, &self.dispatch, dirty);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_mipmap"),
            });
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_mipmap"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);

        for src_level in 0..self.mipmap_levels - 1 {
            let (src, dst) = rect_to_chunks(dirty, src_level, self.chunk_size);
            let scale = 1u32 << src_level as u32;
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let src_key = (x, y, src_level);
                    let dst_key = upper_chunk_of(src_key);

                    let Some(src_chunk) = self.chunks.get(&src_key) else {
                        continue;
                    };
                    let Some(dst_chunk) = self.chunks.get(&dst_key) else {
                        continue;
                    };

                    cpass.set_bind_group(1, Some(&dst_chunk.draw), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.draw), &[]);
                    dispatch_workgroups(&mut cpass, dirty.extend / scale);
                }
            }
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn merge_from(&self, scratch: &Layer, dirty: Rectangle, erase: bool) {
        write_dispatch_uniform(&self.queue, &self.dispatch, dirty);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_merge"),
            });
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_merge"),
            timestamp_writes: None,
        });

        match erase {
            false => cpass.set_pipeline(&self.merge_pipelines.over),
            true => cpass.set_pipeline(&self.merge_pipelines.erase),
        }

        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);

        let (src, dst) = rect_to_chunks(dirty, 0, self.chunk_size);
        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                let key = (x, y, 0);

                let Some(main_chunk) = self.chunks.get(&key) else {
                    continue;
                };

                let Some(scratch_chunk) = scratch.chunks.get(&key) else {
                    continue;
                };

                cpass.set_bind_group(1, Some(&scratch_chunk.draw), &[]);
                cpass.set_bind_group(2, Some(&main_chunk.draw), &[]);
                dispatch_workgroups(&mut cpass, dirty.extend);
            }
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn recycle_chunk(&mut self, chunk: &Chunk, key: ChunkKey) {
        self.queue.write_buffer(
            &chunk.key,
            0,
            bytes_of(&ChunkUniform {
                chunk: [key.0, key.1, key.2 as i32],
                _pad: 0,
            }),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_clear"),
            });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_clear"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.clear_pipeline);
        cpass.set_bind_group(0, Some(&chunk.draw), &[]);
        dispatch_workgroups(&mut cpass, UVec2::splat(self.chunk_size));

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn render(&self, rpass: &mut RenderPass, camera: &Camera, debug: bool, erase: bool) {
        let view_rect = camera.world_view_rect();
        let mipmap = mipmap_floor(camera.zoom);
        let actual_mipmap = mipmap.min(self.mipmap_levels.saturating_sub(1));
        let (src, dst) = rect_to_chunks(view_rect, actual_mipmap, self.chunk_size);

        match (debug, erase) {
            (false, false) => rpass.set_pipeline(&self.render_pipelines.over),
            (true, false) => rpass.set_pipeline(&self.render_pipelines.over_debug),
            (_, true) => rpass.set_pipeline(&self.render_pipelines.erase),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        if camera.zoom.q32_as_f64().exp2() > 6.0 {
            rpass.set_bind_group(1, &self.sampler_group_unfiltered, &[]);
        } else {
            rpass.set_bind_group(1, &self.sampler_group_filtered, &[]);
        }

        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                if let Some(chunk) = self.chunks.get(&(x, y, actual_mipmap)) {
                    rpass.set_bind_group(2, &chunk.render, &[]);
                    rpass.draw(0..4, 0..1);
                }
            }
        }
    }

    fn create_empty_chunk(&mut self, key: ChunkKey) -> Chunk {
        if let Some(chunk) = self.pool.pop() {
            self.recycle_chunk(&chunk, key);
            chunk
        } else {
            let texture = create_chunk_texture(&self.device, self.chunk_size);
            self.create_chunk(texture, key)
        }
    }

    fn create_chunk(&self, texture: Texture, key: ChunkKey) -> Chunk {
        create_chunk(
            &self.device,
            self.chunk_size,
            &self.chunk_render_layout,
            &self.chunk_draw_layout,
            texture,
            key,
        )
    }
}

fn dispatch_workgroups(cpass: &mut ComputePass, size: UVec2) {
    cpass.dispatch_workgroups(
        size.x.saturating_sub(1) / WORKGROUP_SIZE.x + 1,
        size.y.saturating_sub(1) / WORKGROUP_SIZE.y + 1,
        1,
    );
}

// --- Utils --- //

fn write_dispatch_uniform(queue: &Queue, buffer: &Buffer, dirty: Rectangle) {
    let uniform = DispatchUniform {
        dispatch_coords: dirty.origin.into(),
        dispatch_size: dirty.extend.into(),
    };
    queue.write_buffer(buffer, 0, bytes_of(&uniform));
}

fn mipmap_floor(zoom: i64) -> u8 {
    (-(zoom.q32_floor() + 1)).max(0) as u8
}

fn rect_to_chunks(rect: Rectangle, mipmap: u8, chunk_size: u32) -> ((i32, i32), (i32, i32)) {
    let size = chunk_size as i32 * (1i32 << mipmap as i32);
    let chunk_src = (rect.left().div_euclid(size), rect.down().div_euclid(size));
    let chunk_dst = (
        (rect.right() - 1).div_euclid(size) + 1,
        (rect.up() - 1).div_euclid(size) + 1,
    );
    (chunk_src, chunk_dst)
}

fn upper_chunk_of(chunk: ChunkKey) -> ChunkKey {
    (chunk.0.div_euclid(2), chunk.1.div_euclid(2), chunk.2 + 1)
}

// --- Chunks --- //

fn create_chunk_texture(device: &Device, chunk_size: u32) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("layer_chunk_texture"),
        size: Extent3d {
            width: chunk_size,
            height: chunk_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::STORAGE_BINDING,
        view_formats: &[TextureFormat::Rgba8UnormSrgb],
    })
}

fn create_chunk(
    device: &Device,
    _chunk_size: u32,
    chunk_render_layout: &BindGroupLayout,
    chunk_draw_layout: &BindGroupLayout,
    texture: Texture,
    key: ChunkKey,
) -> Chunk {
    let key_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("layer_chunk_key"),
        contents: bytes_of(&ChunkUniform {
            chunk: [key.0, key.1, key.2 as i32],
            _pad: 0,
        }),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    let texture_fragment_view = texture.create_view(&TextureViewDescriptor {
        label: Some("layer_chunk_texture_view"),
        format: Some(TextureFormat::Rgba8UnormSrgb),
        usage: Some(TextureUsages::TEXTURE_BINDING),
        ..Default::default()
    });

    let texture_compute_view = texture.create_view(&TextureViewDescriptor {
        label: Some("layer_chunk_texture_view"),
        format: Some(TextureFormat::Rgba8Unorm),
        usage: Some(TextureUsages::STORAGE_BINDING),
        ..Default::default()
    });

    let render_bind = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_render"),
        layout: &chunk_render_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &key_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&texture_fragment_view),
            },
        ],
    });

    let draw_bind = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_draw"),
        layout: &chunk_draw_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_compute_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &key_buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    Chunk {
        key: key_buffer,
        texture,
        render: render_bind,
        draw: draw_bind,
    }
}

// --- Layouts --- //

const LAYOUT_DISPATCH: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_dispatch"),
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
};

const LAYOUT_SAMPLER: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_sampler"),
    entries: &[BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    }],
};

/// Contains read_write storage texture of chunk in format `Rgba8Unorm` and chunk key in vec3
const LAYOUT_CHUNK_DRAW: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_chunk_draw"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::ReadWrite,
                format: TextureFormat::Rgba8Unorm,
                view_dimension: TextureViewDimension::D2,
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
    ],
};

/// Contains drawing rectangle and texture of chunk in format `Rgba8UnormSrgb`
const LAYOUT_CHUNK_RENDER: BindGroupLayoutDescriptor = BindGroupLayoutDescriptor {
    label: Some("layer_chunk_render"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
    ],
};

// --- Bind Groups --- //

fn dispatch_group(device: &Device, dispatch_layout: &BindGroupLayout) -> (Buffer, BindGroup) {
    let dispatch = device.create_buffer(&BufferDescriptor {
        label: Some("layer_dispatch"),
        size: size_of::<DispatchUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let dispatch_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_dispatch"),
        layout: dispatch_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &dispatch,
                offset: 0,
                size: None,
            }),
        }],
    });

    (dispatch, dispatch_group)
}

fn sampler_groups(device: &Device, sampler_layout: &BindGroupLayout) -> (BindGroup, BindGroup) {
    let sampler_unfiltered = device.create_sampler(&SamplerDescriptor {
        label: Some("layer_sampler_unfiltered"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..Default::default()
    });

    let sampler_filtered = device.create_sampler(&SamplerDescriptor {
        label: Some("layer_sampler_filtered"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });

    let sampler_group_unfiltered = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_sampler_unfiltered"),
        layout: sampler_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Sampler(&sampler_unfiltered),
        }],
    });

    let sampler_group_filtered = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_sampler_filtered"),
        layout: sampler_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Sampler(&sampler_filtered),
        }],
    });

    (sampler_group_unfiltered, sampler_group_filtered)
}

// --- Pipelines --- //

fn render_pipelines(
    config: &LayerConfig,
    device: &Device,
    sampler_layout: BindGroupLayout,
    chunk_render_layout: &BindGroupLayout,
) -> RenderPipelines {
    let render_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_chunk"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}",
                include_str!("widgets/renderer/lib_camera.wgsl"),
                include_str!("layer/chunk.wgsl"),
            )
            .into(),
        ),
    });

    let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_chunk"),
        bind_group_layouts: &[
            &config.camera_bind_layout,
            &sampler_layout,
            chunk_render_layout,
        ],
        immediate_size: 0,
    });

    let new_pipeline = |blend: BlendState, label: &str, fs_entry: &str| {
        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &render_shader,
                entry_point: Some(fs_entry),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.surface_format,
                    blend: Some(blend),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        })
    };

    RenderPipelines {
        over: new_pipeline(BlendState::ALPHA_BLENDING, "layer_chunk_over", "fs_main"),
        over_debug: new_pipeline(
            BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer_chunk_over_debug",
            "fs_main_debug",
        ),
        erase: new_pipeline(
            BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            },
            "layer_chunk_erase",
            "fs_main",
        ),
    }
}

fn mipmap_pipeline(
    device: &Device,
    chunk_draw_layout: &BindGroupLayout,
    dispatch_layout: &BindGroupLayout,
) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_mipmap"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("stroke/lib_dispatch.wgsl"),
                include_str!("stroke/lib_colorspace.wgsl"),
                include_str!("stroke/mipmap.wgsl"),
            )
            .into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_mipmap"),
        bind_group_layouts: &[dispatch_layout, chunk_draw_layout, chunk_draw_layout],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("layer_mipmap"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn merge_pipelines(
    device: &Device,
    chunk_draw_layout: &BindGroupLayout,
    dispatch_layout: &BindGroupLayout,
) -> MergePipelines {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_merge"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("stroke/lib_colorspace.wgsl"),
                include_str!("stroke/lib_dispatch.wgsl"),
                include_str!("layer/merge.wgsl"),
            )
            .into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_merge"),
        bind_group_layouts: &[dispatch_layout, chunk_draw_layout, chunk_draw_layout],
        immediate_size: 0,
    });

    let new_pipeline = |label, cs_entry| {
        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some(cs_entry),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        })
    };

    MergePipelines {
        over: new_pipeline("layer_merge", "cs_main"),
        erase: new_pipeline("layer_merge_erase", "cs_erase"),
    }
}

fn clear_pipeline(device: &Device, chunk_draw_layout: &BindGroupLayout) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_clear"),
        source: ShaderSource::Wgsl(include_str!("layer/clear.wgsl").into()),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_clear"),
        bind_group_layouts: &[chunk_draw_layout],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("layer_clear"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
}
