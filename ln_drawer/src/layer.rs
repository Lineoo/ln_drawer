pub mod brush;
pub mod dirty;
pub mod interpolate;
pub mod modifier;
pub mod shape;
pub mod stream;
pub mod wrapper;

use std::mem::size_of;

use bytemuck::bytes_of;
use glam::{IVec2, UVec2};
use hashbrown::HashMap;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendComponent,
    BlendFactor, BlendOperation, BlendState, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    ComputePass, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device,
    Extent3d, FilterMode, FragmentState, Origin3d, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, StorageTextureAccess, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
};

pub type ChunkKey = (i32, i32, u8);

pub const DEFAULT_MIPMAP_DISABLED: u32 = 1;
pub const DEFAULT_MIPMAP_ENABLED: u32 = 8;
pub const DEFAULT_CHUNK_SIZE: u32 = 512;

const WORKGROUP_SIZE: UVec2 = UVec2::new(16, 16);

// function: render, merge, mipmap, clear & chunk recycle
pub struct LayerPipeline {
    device: Device,
    queue: Queue,

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

pub struct Layer {
    pub chunks: HashMap<ChunkKey, Chunk>,
    pub chunk_size: u32,
    pub mipmap_levels: u8,
    pub controlled: bool,
}

pub struct ChunkPool {
    pub list: Vec<Chunk>,
    pub chunk_size: u32,
}

#[derive(Clone)]
pub struct Chunk {
    pub key: Buffer,
    pub texture: Texture,
    pub render: BindGroup,
    pub draw: BindGroup,
}

struct RenderPipelines {
    over: RenderPipeline,
    over_debug: RenderPipeline,
    replace: RenderPipeline,
    replace_debug: RenderPipeline,
    #[expect(unused)]
    erase: RenderPipeline,
}

struct MergePipelines {
    over: ComputePipeline,
    #[expect(unused)]
    erase: ComputePipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    coords: [i32; 2],
    size: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    coords: [i32; 2],
    size: [u32; 2],
}

impl LayerPipeline {
    pub fn new(
        device: Device,
        queue: Queue,
        surface_format: TextureFormat,
        camera_bind_layout: &BindGroupLayout,
    ) -> Self {
        let chunk_render_layout = device.create_bind_group_layout(&LAYOUT_CHUNK_RENDER);
        let chunk_draw_layout = device.create_bind_group_layout(&LAYOUT_CHUNK_DRAW);
        let dispatch_layout = device.create_bind_group_layout(&LAYOUT_DISPATCH);
        let sampler_layout = device.create_bind_group_layout(&LAYOUT_SAMPLER);

        let (dispatch, dispatch_group) = dispatch_group(&device, &dispatch_layout);

        let (sampler_group_unfiltered, sampler_group_filtered) =
            sampler_groups(&device, &sampler_layout);

        let render_pipelines = render_pipelines(
            &device,
            surface_format,
            camera_bind_layout,
            sampler_layout,
            &chunk_render_layout,
        );

        let merge_pipelines = merge_pipelines(&device, &chunk_draw_layout, &dispatch_layout);
        let mipmap_pipeline = mipmap_pipeline(&device, &chunk_draw_layout, &dispatch_layout);
        let clear_pipeline = clear_pipeline(&device, &chunk_draw_layout);

        LayerPipeline {
            device,
            queue,
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

    /// Assume `self.controlled` is false. if `origin` is `None`, create transparent chunk, otherwise
    /// clone data from the `origin` layer
    pub fn prepare_chunks(
        &mut self,
        dst: &mut Layer,
        src: Option<&Layer>,
        pool: &mut ChunkPool,
        rect: Rectangle,
    ) {
        debug_assert!(!dst.controlled, "controlled layer cannot prepare chunks");
        debug_assert_eq!(
            dst.chunk_size, pool.chunk_size,
            "pool chunk_size does not matched"
        );

        let mut chunks = Vec::new();

        for mipmap in 0..dst.mipmap_levels {
            let (start, end) = rect_to_chunks(rect, mipmap, dst.chunk_size);
            for chunk_x in start.0..end.0 {
                for chunk_y in start.1..end.1 {
                    let key = (chunk_x, chunk_y, mipmap);
                    if !dst.chunks.contains_key(&key) {
                        chunks.push(key);
                    }
                }
            }
        }

        for key in chunks {
            let dst_chunk = self.recycle_empty_chunk(key, dst.chunk_size, pool);

            if let Some(src) = src
                && let Some(src_chunk) = src.chunks.get(&key)
            {
                let mut encoder = self
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("layer_prepare_copy"),
                    });
                encoder.copy_texture_to_texture(
                    TexelCopyTextureInfo {
                        texture: &src_chunk.texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    TexelCopyTextureInfo {
                        texture: &dst_chunk.texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: dst.chunk_size,
                        height: dst.chunk_size,
                        depth_or_array_layers: 1,
                    },
                );
                self.queue.submit([encoder.finish()]);
            }

            dst.chunks.insert(key, dst_chunk);
        }
    }

    pub fn generate_mipmaps(&self, layer: &Layer, dirty: Rectangle) {
        if layer.mipmap_levels <= 1 {
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

        for src_level in 0..layer.mipmap_levels - 1 {
            let (src, dst) = rect_to_chunks(dirty, src_level, layer.chunk_size);
            let scale = 1u32 << src_level;
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let src_key = (x, y, src_level);
                    let dst_key = upper_chunk_of(src_key);

                    let Some(src_chunk) = layer.chunks.get(&src_key) else {
                        continue;
                    };
                    let Some(dst_chunk) = layer.chunks.get(&dst_key) else {
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

    pub fn merge_from(
        &self,
        layer: &Layer,
        scratch: &Layer,
        dirty: Rectangle,
        chunks: &[ChunkKey],
    ) {
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

        cpass.set_pipeline(&self.merge_pipelines.over);
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);

        // XXX won't work if two layers' chunk_size unmatched
        for &key in chunks {
            let Some(main_chunk) = layer.chunks.get(&key) else {
                continue;
            };

            let Some(scratch_chunk) = scratch.chunks.get(&key) else {
                continue;
            };

            cpass.set_bind_group(1, Some(&main_chunk.draw), &[]);
            cpass.set_bind_group(2, Some(&scratch_chunk.draw), &[]);
            dispatch_workgroups(&mut cpass, dirty.extend);
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn write_chunk_key(&mut self, chunk: &Chunk, key: ChunkKey, chunk_size: u32) {
        let rect = chunk_to_rect(key, chunk_size);
        let uniform = ChunkUniform {
            coords: rect.origin.into(),
            size: rect.extend.into(),
        };
        self.queue.write_buffer(&chunk.key, 0, bytes_of(&uniform));
    }

    pub fn clear_chunk(&mut self, chunk: &Chunk, chunk_size: u32) {
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
        dispatch_workgroups(&mut cpass, UVec2::splat(chunk_size));

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn render(
        &self,
        layer: &Layer,
        rpass: &mut RenderPass,
        camera: &Camera,
        debug: bool,
        replace: bool,
    ) {
        let view_rect = camera.world_view_rect();
        let mipmap = mipmap_floor(camera.zoom);
        let actual_mipmap = mipmap.min(layer.mipmap_levels.saturating_sub(1));
        let (src, dst) = rect_to_chunks(view_rect, actual_mipmap, layer.chunk_size);

        match (debug, replace) {
            (false, false) => rpass.set_pipeline(&self.render_pipelines.over),
            (true, false) => rpass.set_pipeline(&self.render_pipelines.over_debug),
            (false, true) => rpass.set_pipeline(&self.render_pipelines.replace),
            (true, true) => rpass.set_pipeline(&self.render_pipelines.replace_debug),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        if camera.zoom.q32_as_f64().exp2() > 6.0 {
            rpass.set_bind_group(1, &self.sampler_group_unfiltered, &[]);
        } else {
            rpass.set_bind_group(1, &self.sampler_group_filtered, &[]);
        }

        for x in src.0..dst.0 {
            for y in src.1..dst.1 {
                if let Some(chunk) = layer.chunks.get(&(x, y, actual_mipmap)) {
                    rpass.set_bind_group(2, &chunk.render, &[]);
                    rpass.draw(0..4, 0..1);
                }
            }
        }
    }

    fn recycle_empty_chunk(
        &mut self,
        key: ChunkKey,
        chunk_size: u32,
        pool: &mut ChunkPool,
    ) -> Chunk {
        if let Some(chunk) = pool.list.pop() {
            self.write_chunk_key(&chunk, key, chunk_size);
            self.clear_chunk(&chunk, chunk_size);
            chunk
        } else {
            let texture = create_chunk_texture(&self.device, chunk_size);

            create_chunk(
                &self.device,
                chunk_size,
                &self.chunk_render_layout,
                &self.chunk_draw_layout,
                texture,
                key,
            )
        }
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
        coords: dirty.origin.into(),
        size: dirty.extend.into(),
    };
    queue.write_buffer(buffer, 0, bytes_of(&uniform));
}

fn mipmap_floor(zoom: i64) -> u8 {
    (-(zoom.q32_floor() + 1)).max(0) as u8
}

fn chunk_to_rect((x, y, z): ChunkKey, chunk_size: u32) -> Rectangle {
    let size = chunk_size << z;
    Rectangle {
        origin: IVec2::new(x, y) * size as i32,
        extend: UVec2::splat(size),
    }
}

fn rect_to_chunks(rect: Rectangle, mipmap: u8, chunk_size: u32) -> ((i32, i32), (i32, i32)) {
    let size = (chunk_size << mipmap) as i32;
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
    chunk_size: u32,
    chunk_render_layout: &BindGroupLayout,
    chunk_draw_layout: &BindGroupLayout,
    texture: Texture,
    key: ChunkKey,
) -> Chunk {
    let rect = chunk_to_rect(key, chunk_size);

    let buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("layer_chunk_buffer"),
        contents: bytes_of(&ChunkUniform {
            coords: rect.origin.into(),
            size: rect.extend.into(),
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
                resource: BindingResource::TextureView(&texture_fragment_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: None,
                }),
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
                    buffer: &buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    Chunk {
        key: buffer,
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
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
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
    device: &Device,
    surface_format: TextureFormat,
    camera_bind_layout: &BindGroupLayout,
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
            Some(&camera_bind_layout),
            Some(&sampler_layout),
            Some(chunk_render_layout),
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
                    format: surface_format,
                    blend: Some(blend),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    };

    RenderPipelines {
        over: new_pipeline(BlendState::ALPHA_BLENDING, "layer_chunk_over", "fs_main"),
        over_debug: new_pipeline(
            BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer_chunk_over_debug",
            "fs_main_debug0",
        ),
        replace: new_pipeline(
            BlendState {
                color: BlendComponent::REPLACE,
                alpha: BlendComponent::REPLACE,
            },
            "layer_chunk_replace",
            "fs_main",
        ),
        replace_debug: new_pipeline(
            BlendState {
                color: BlendComponent::REPLACE,
                alpha: BlendComponent::REPLACE,
            },
            "layer_chunk_replace",
            "fs_main_debug1",
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
                "{}{}",
                include_str!("layer/lib_colorspace.wgsl"),
                include_str!("layer/mipmap.wgsl"),
            )
            .into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_mipmap"),
        bind_group_layouts: &[
            Some(dispatch_layout),
            Some(chunk_draw_layout),
            Some(chunk_draw_layout),
        ],
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
                "{}{}",
                include_str!("layer/lib_colorspace.wgsl"),
                include_str!("layer/merge.wgsl"),
            )
            .into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_merge"),
        bind_group_layouts: &[
            Some(dispatch_layout),
            Some(chunk_draw_layout),
            Some(chunk_draw_layout),
        ],
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
        bind_group_layouts: &[Some(chunk_draw_layout)],
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
