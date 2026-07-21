use std::mem::size_of;

use bytemuck::bytes_of;
use glam::{IVec2, UVec2};
use hashbrown::HashMap;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, FilterMode, FragmentState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPass, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::{MSAA_STATE, camera::Camera, vertex::VertexUniform},
};

pub type ChunkKey = (i32, i32, u8);

pub const DEFAULT_CHUNK_SIZE: u32 = 512;

const WORKGROUP_SIZE: UVec2 = UVec2::new(16, 16);

pub struct LayerConfig {
    pub device: Device,
    pub queue: Queue,
    pub surface_format: TextureFormat,
    pub mipmap_levels: u8,
    pub chunk_size: u32,
    pub camera_bind_layout: BindGroupLayout,
}

pub struct Layer {
    device: Device,
    queue: Queue,
    #[allow(dead_code)]
    surface_format: TextureFormat,
    chunk_size: u32,
    mipmap_levels: u8,

    #[allow(dead_code)]
    camera_bind_layout: BindGroupLayout,

    pub render_debugging: bool,

    chunks: HashMap<ChunkKey, Chunk>,

    chunk_render_layout: BindGroupLayout,
    chunk_draw_layout: BindGroupLayout,

    mipmap_pipeline: ComputePipeline,
    dispatch: Buffer,
    dispatch_group: BindGroup,

    render_pipeline: RenderPipeline,
    render_debug_pipeline: RenderPipeline,
    sampler_group_unfiltered: BindGroup,
    sampler_group_filtered: BindGroup,
}

struct Chunk {
    bind: ChunkBind,
}

pub struct ChunkBind {
    pub texture: Texture,
    pub render: BindGroup,
    pub draw: BindGroup,
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

        let chunk_draw_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
        });

        let dispatch_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
        });

        let dispatch = device.create_buffer(&BufferDescriptor {
            label: Some("layer_dispatch"),
            size: size_of::<DispatchUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dispatch_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_dispatch"),
            layout: &dispatch_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &dispatch,
                    offset: 0,
                    size: None,
                }),
            }],
        });

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

        let sampler_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("layer_sampler"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            }],
        });

        let sampler_group_unfiltered = device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_sampler_unfiltered"),
            layout: &sampler_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Sampler(&sampler_unfiltered),
            }],
        });

        let sampler_group_filtered = device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_sampler_filtered"),
            layout: &sampler_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Sampler(&sampler_filtered),
            }],
        });

        let chunk_render_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
        });

        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("layer_chunk"),
            source: ShaderSource::Wgsl(
                format!(
                    "{}{}",
                    include_str!("widgets/renderer/lib_camera.wgsl"),
                    include_str!("layer_chunk.wgsl"),
                )
                .into(),
            ),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("layer_chunk"),
            bind_group_layouts: &[
                &config.camera_bind_layout,
                &sampler_layout,
                &chunk_render_layout,
            ],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("layer_chunk"),
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
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        });

        let render_debug_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("layer_chunk_debug"),
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
                entry_point: Some("fs_main_debug"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: config.surface_format,
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        });

        let mipmap_pipeline =
            Self::create_mipmap_pipeline(device, &chunk_draw_layout, &dispatch_layout);

        Layer {
            device: config.device,
            queue: config.queue,
            surface_format: config.surface_format,
            chunk_size: config.chunk_size,
            mipmap_levels: config.mipmap_levels,
            camera_bind_layout: config.camera_bind_layout,
            render_debugging: false,
            chunks: HashMap::new(),
            chunk_render_layout,
            chunk_draw_layout,
            mipmap_pipeline,
            dispatch,
            dispatch_group,
            render_pipeline,
            render_debug_pipeline,
            sampler_group_unfiltered,
            sampler_group_filtered,
        }
    }

    fn create_mipmap_pipeline(
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

    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    pub fn mipmap_levels(&self) -> u8 {
        self.mipmap_levels
    }

    pub fn lower_mipmap_of(zoom: i64) -> u8 {
        (-(zoom.q32_floor() + 1)).max(0) as u8
    }

    pub fn chunk_rect(key: ChunkKey, chunk_size: u32) -> Rectangle {
        let size = chunk_size as i32 * (1i32 << key.2 as i32);
        Rectangle {
            origin: IVec2::new(key.0 * size, key.1 * size),
            extend: UVec2::splat(size as u32),
        }
    }

    pub fn chunks_within(
        view_rect: Rectangle,
        mipmap: u8,
        chunk_size: u32,
    ) -> ((i32, i32), (i32, i32)) {
        let size = chunk_size as i32 * (1i32 << mipmap as i32);
        let chunk_src = (
            view_rect.left().div_euclid(size),
            view_rect.down().div_euclid(size),
        );
        let chunk_dst = (
            (view_rect.right() - 1).div_euclid(size) + 1,
            (view_rect.up() - 1).div_euclid(size) + 1,
        );
        (chunk_src, chunk_dst)
    }

    pub fn upper_chunk_of(chunk: ChunkKey) -> ChunkKey {
        (chunk.0.div_euclid(2), chunk.1.div_euclid(2), chunk.2 + 1)
    }

    pub fn chunk_of(center: IVec2, zoom: i64, chunk_size: u32) -> ChunkKey {
        let mipmap = Self::lower_mipmap_of(zoom);
        let size = chunk_size as i32 * (1i32 << mipmap as i32);
        (center.x.div_euclid(size), center.y.div_euclid(size), mipmap)
    }

    pub fn ensure_chunks(&mut self, rect: Rectangle) -> Vec<ChunkKey> {
        let mut created = Vec::new();
        for mipmap in 0..self.mipmap_levels {
            let (src, dst) = Self::chunks_within(rect, mipmap, self.chunk_size);
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let key = (x, y, mipmap);
                    if !self.chunks.contains_key(&key) {
                        let bind = self.create_chunk(key);
                        self.chunks.insert(key, Chunk { bind });
                        created.push(key);
                    }
                }
            }
        }
        created
    }

    pub fn insert_texture(&mut self, key: ChunkKey, texture: Texture) {
        let bind = self.create_chunk_from_texture(texture, key);
        self.chunks.insert(key, Chunk { bind });
    }

    pub fn take_chunk(&mut self, key: ChunkKey) -> Option<Texture> {
        self.chunks.remove(&key).map(|c| c.bind.texture)
    }

    pub fn has_chunk(&self, key: ChunkKey) -> bool {
        self.chunks.contains_key(&key)
    }

    pub fn draw_bind(&self, key: ChunkKey) -> Option<&BindGroup> {
        self.chunks.get(&key).map(|c| &c.bind.draw)
    }

    pub fn chunk_bind(&self, key: ChunkKey) -> Option<&ChunkBind> {
        self.chunks.get(&key).map(|c| &c.bind)
    }

    pub fn chunks(&self) -> impl Iterator<Item = (ChunkKey, &ChunkBind)> {
        self.chunks.iter().map(|(&k, c)| (k, &c.bind))
    }

    pub fn clear(&mut self) -> Vec<(ChunkKey, Texture)> {
        self.chunks
            .drain()
            .map(|(k, c)| (k, c.bind.texture))
            .collect()
    }

    pub fn generate_mipmaps(&mut self, dirty: Rectangle) {
        if self.mipmap_levels <= 1 {
            return;
        }

        self.upload_dispatch(dirty);

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
            let (src, dst) = Self::chunks_within(dirty, src_level, self.chunk_size);
            let scale = 1u32 << src_level as u32;
            for x in src.0..dst.0 {
                for y in src.1..dst.1 {
                    let src_key = (x, y, src_level);
                    let dst_key = Self::upper_chunk_of(src_key);

                    let Some(src_chunk) = self.chunks.get(&src_key) else {
                        continue;
                    };
                    let Some(dst_chunk) = self.chunks.get(&dst_key) else {
                        continue;
                    };

                    cpass.set_bind_group(1, Some(&dst_chunk.bind.draw), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.bind.draw), &[]);
                    cpass.dispatch_workgroups(
                        dirty.extend.x.saturating_sub(1) / scale / WORKGROUP_SIZE.x + 1,
                        dirty.extend.y.saturating_sub(1) / scale / WORKGROUP_SIZE.y + 1,
                        1,
                    );
                }
            }
        }

        drop(cpass);
        self.queue.submit([encoder.finish()]);
    }

    pub fn set_mipmap_levels(&mut self, levels: u8) {
        assert!(levels >= 1, "mipmap_levels must be >= 1");
        self.mipmap_levels = levels;
    }

    /// Create a camera bind group for use at @group(0) of the render pipeline.
    /// The caller should pass the camera's uniform buffer.
    pub fn create_camera_group(&self, camera_buffer: &Buffer) -> BindGroup {
        self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_camera"),
            layout: &self.camera_bind_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: camera_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        })
    }

    pub fn render(&self, rpass: &mut RenderPass<'_>, camera: &Camera) {
        let view_rect = camera.world_view_rect();
        let mipmap = Self::lower_mipmap_of(camera.zoom);
        let actual_mipmap = mipmap.min(self.mipmap_levels.saturating_sub(1));
        let (src, dst) = Self::chunks_within(view_rect, actual_mipmap, self.chunk_size);

        match self.render_debugging {
            false => rpass.set_pipeline(&self.render_pipeline),
            true => rpass.set_pipeline(&self.render_debug_pipeline),
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
                    rpass.set_bind_group(2, &chunk.bind.render, &[]);
                    rpass.draw(0..4, 0..1);
                }
            }
        }
    }

    pub fn chunk_draw_layout(&self) -> &BindGroupLayout {
        &self.chunk_draw_layout
    }

    pub fn chunk_render_layout(&self) -> &BindGroupLayout {
        &self.chunk_render_layout
    }

    fn create_chunk(&self, key: ChunkKey) -> ChunkBind {
        let texture = self.device.create_texture(&TextureDescriptor {
            label: Some("layer_chunk_texture"),
            size: Extent3d {
                width: self.chunk_size,
                height: self.chunk_size,
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
        });

        self.create_chunk_from_texture(texture, key)
    }

    fn create_chunk_from_texture(&self, texture: Texture, key: ChunkKey) -> ChunkBind {
        let rect = Self::chunk_rect(key, self.chunk_size);

        let rectangle = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("layer_chunk_rectangle"),
            contents: bytes_of(&VertexUniform {
                origin: rect.origin.into(),
                extend: rect.extend.into(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let key_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
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

        let render_bind = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_chunk_render"),
            layout: &self.chunk_render_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &rectangle,
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

        let draw_bind = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_chunk_draw"),
            layout: &self.chunk_draw_layout,
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

        ChunkBind {
            texture,
            render: render_bind,
            draw: draw_bind,
        }
    }

    fn upload_dispatch(&self, dirty: Rectangle) {
        let uniform = DispatchUniform {
            dispatch_coords: dirty.origin.into(),
            dispatch_size: dirty.extend.into(),
        };
        self.queue
            .write_buffer(&self.dispatch, 0, bytes_of(&uniform));
    }
}
