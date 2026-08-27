pub mod brush;
pub mod input;
pub mod stream;
pub mod traveler;
pub mod wrapper;

use std::mem::size_of;

use bytemuck::bytes_of;
use glam::{IVec2, UVec2};
use hashbrown::HashMap;
use wgpu::{
    Adapter, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, FilterMode, FragmentState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPass, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureFormatFeatureFlags,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    widgets::shaders::{LIB_CAMERA, LIB_COLORSPACE, LIB_CONSTANT, LIB_RECTANGLE, shader_compile},
};

pub type ChunkKey = (i32, i32, u8);

pub const DEFAULT_MIPMAP_DISABLED: u8 = 1;
pub const DEFAULT_MIPMAP_ENABLED: u8 = 8;
pub const DEFAULT_CHUNK_SIZE: u32 = 512;

pub const CHUNK_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

const DISPATCH_CAPACITY: u64 = 4;
const DRAWS_ARRAY_CAPACITY: u64 = 0x2000;
const WORKGROUP_SIZE: UVec2 = UVec2::new(16, 16);

// function: render, merge, mipmap, clear & chunk recycle
pub struct LayerPipeline {
    _adapter: Adapter,
    device: Device,
    queue: Queue,

    chunk_layout: ChunkLayout,

    dispatch: Buffer,
    dispatch_group: BindGroup,

    sampler_group_unfiltered: BindGroup,
    sampler_group_filtered: BindGroup,

    draws_dispatch: Buffer,
    draws_dispatch_group: BindGroup,
    draws_length: Buffer,
    draws_array: Buffer,

    render_pipelines: RenderPipelines,
    merge_pipelines: MergePipelines,
    mipmap_pipeline: ComputePipeline,
    copy_pipeline: ComputePipeline,
    clear_pipeline: ComputePipeline,
    brush_pipelines: BrushPipelines,

    support_read_write: bool,
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
    pub rectangle: Buffer,
    pub texture: Texture,
    pub dispatch: BindGroup,
    pub render: BindGroup,
    pub read: BindGroup,
    pub write: BindGroup,
    /// Same to `read` if not supported.
    pub read_write: BindGroup,
}

#[derive(Clone)]
pub struct ChunkLayout {
    dispatch: BindGroupLayout,
    render: BindGroupLayout,
    read: BindGroupLayout,
    write: BindGroupLayout,
    read_write: BindGroupLayout,
}

struct RenderPipelines {
    over: RenderPipeline,
    over_fast: RenderPipeline,
    over_debug: RenderPipeline,
    replace: RenderPipeline,
    replace_fast: RenderPipeline,
    replace_debug: RenderPipeline,
}

struct MergePipelines {
    over: ComputePipeline,
    #[expect(unused)]
    replace: ComputePipeline,
    #[expect(unused)]
    erase: ComputePipeline,
}

struct BrushPipelines {
    blur: ComputePipeline,
    round_over: ComputePipeline,
    round_erase: ComputePipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    coords: [i32; 2],
    size: [u32; 2],
}

impl LayerPipeline {
    pub fn new(
        _adapter: Adapter,
        device: Device,
        queue: Queue,
        surface_format: TextureFormat,
        camera_bind_layout: &BindGroupLayout,
    ) -> Self {
        let texture_features = _adapter.get_texture_format_features(CHUNK_TEXTURE_FORMAT);
        let support_read_write =
            (texture_features.flags).contains(TextureFormatFeatureFlags::STORAGE_READ_WRITE);

        log::debug!("texture read write: {support_read_write}");

        let dispatch_layout = device.create_bind_group_layout(&LAYOUT_DISPATCH);
        let (dispatch, dispatch_group) = dispatch_group(&device, &dispatch_layout);

        let sampler_layout = device.create_bind_group_layout(&LAYOUT_SAMPLER);
        let (sampler_group_unfiltered, sampler_group_filtered) =
            sampler_groups(&device, &sampler_layout);

        let draw_dispatch_layout = device.create_bind_group_layout(&LAYOUT_DRAW_DISPATCH);
        let (draws_dispatch, draws_length, draws_array, draws_dispatch_group) =
            draws_dispatch_group(&device, &draw_dispatch_layout);

        let chunk_layout = ChunkLayout {
            dispatch: dispatch_layout,
            render: device.create_bind_group_layout(&LAYOUT_CHUNK_RENDER),
            read: device.create_bind_group_layout(&LAYOUT_CHUNK_READ),
            write: device.create_bind_group_layout(&LAYOUT_CHUNK_WRITE),
            read_write: match support_read_write {
                true => device.create_bind_group_layout(&LAYOUT_CHUNK_READ_WRITE),
                false => device.create_bind_group_layout(&LAYOUT_CHUNK_READ),
            },
        };

        let render_pipelines = render_pipelines(
            &device,
            surface_format,
            camera_bind_layout,
            sampler_layout,
            &chunk_layout.render,
        );

        let brush_pipelines = brush_pipelines(
            &device,
            support_read_write,
            &draw_dispatch_layout,
            &chunk_layout,
        );
        let merge_pipelines = merge_pipelines(&device, support_read_write, &chunk_layout);
        let mipmap_pipeline = mipmap_pipeline(&device, &chunk_layout);
        let copy_pipeline = copy_pipeline(&device, &chunk_layout);
        let clear_pipeline = clear_pipeline(&device, &chunk_layout);

        LayerPipeline {
            _adapter,
            device,
            queue,
            chunk_layout,
            dispatch,
            dispatch_group,
            sampler_group_unfiltered,
            sampler_group_filtered,
            draws_dispatch,
            draws_dispatch_group,
            draws_length,
            draws_array,
            render_pipelines,
            merge_pipelines,
            mipmap_pipeline,
            copy_pipeline,
            clear_pipeline,
            brush_pipelines,
            support_read_write,
        }
    }

    pub fn validate_chunks(&self, dst: &mut Layer, rect: Rectangle) -> bool {
        for mipmap in 0..dst.mipmap_levels {
            let (start, end) = rect_to_chunks(rect, mipmap, dst.chunk_size);
            for chunk_x in start.0..end.0 {
                for chunk_y in start.1..end.1 {
                    let key = (chunk_x, chunk_y, mipmap);
                    if !dst.chunks.contains_key(&key) {
                        return false;
                    }
                }
            }
        }

        return true;
    }

    /// Assume `self.controlled` is false. if `origin` is `None`, create transparent chunk, otherwise
    /// clone data from the `origin` layer
    pub fn prepare_chunks(
        &self,
        dst: &mut Layer,
        src: Option<&Layer>,
        pool: &mut ChunkPool,
        rect: Rectangle,
        cpass: &mut ComputePass,
    ) {
        debug_assert!(!dst.controlled, "controlled layer cannot prepare chunks");
        debug_assert_eq!(
            dst.chunk_size, pool.chunk_size,
            "pool chunk_size does not matched"
        );

        let mut dst_chunks = Vec::new();

        for mipmap in 0..dst.mipmap_levels {
            let (start, end) = rect_to_chunks(rect, mipmap, dst.chunk_size);
            for chunk_x in start.0..end.0 {
                for chunk_y in start.1..end.1 {
                    let key = (chunk_x, chunk_y, mipmap);
                    if !dst.chunks.contains_key(&key) {
                        dst_chunks.push(key);
                    }
                }
            }
        }

        // Copy texture if src layer is provided
        if let Some(src) = src {
            debug_assert_eq!(
                src.chunk_size, dst.chunk_size,
                "reference layer chunk_size does not matched"
            );

            for dst_key in dst_chunks {
                let src_chunk = src.chunks.get(&dst_key);
                let dst_chunk = self.recycle_chunk(dst_key, dst.chunk_size, pool);

                if let Some(src_chunk) = src_chunk {
                    cpass.set_pipeline(&self.copy_pipeline);
                    cpass.set_bind_group(0, &dst_chunk.dispatch, &[0]);
                    cpass.set_bind_group(1, &dst_chunk.write, &[]);
                    cpass.set_bind_group(2, &src_chunk.read, &[]);
                    let chunk_rect = chunk_to_rect(dst_key, dst.chunk_size);
                    dispatch_workgroups(cpass, &[chunk_rect]);
                }

                dst.chunks.insert(dst_key, dst_chunk);
            }
        } else {
            for dst_key in dst_chunks {
                let dst_chunk = self.recycle_chunk(dst_key, dst.chunk_size, pool);
                dst.chunks.insert(dst_key, dst_chunk);
            }
        }
    }

    pub fn generate_mipmaps(&self, layer: &Layer, dirty: Rectangle) {
        if layer.mipmap_levels <= 1 {
            return;
        }

        write_dispatch(&self.queue, &self.dispatch, 0, dirty);

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
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[0]);

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

                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    let dst_rect = chunk_to_rect(dst_key, layer.chunk_size);
                    let src_rect = chunk_to_rect(src_key, layer.chunk_size);
                    dispatch_workgroups_divide(&mut cpass, &[dirty, dst_rect, src_rect], scale);
                }
            }
        }

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
        let mipmap = (-camera.zoom).q32_floor().max(0) as u8;
        let actual_mipmap = mipmap.min(layer.mipmap_levels.saturating_sub(1));
        let (src, dst) = rect_to_chunks(view_rect, actual_mipmap, layer.chunk_size);
        let pixel = camera.zoom.q32_as_f64().exp2() > 6.0;

        match (debug, replace, pixel) {
            (false, false, false) => rpass.set_pipeline(&self.render_pipelines.over),
            (false, false, true) => rpass.set_pipeline(&self.render_pipelines.over_fast),
            (true, false, _) => rpass.set_pipeline(&self.render_pipelines.over_debug),
            (false, true, false) => rpass.set_pipeline(&self.render_pipelines.replace),
            (false, true, true) => rpass.set_pipeline(&self.render_pipelines.replace_fast),
            (true, true, _) => rpass.set_pipeline(&self.render_pipelines.replace_debug),
        }

        rpass.set_bind_group(0, &camera.bind, &[]);

        match pixel {
            true => rpass.set_bind_group(1, &self.sampler_group_unfiltered, &[]),
            false => rpass.set_bind_group(1, &self.sampler_group_filtered, &[]),
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

    /// return `true` if the chunk is guaranteed to be empty
    fn recycle_chunk(&self, key: ChunkKey, chunk_size: u32, pool: &mut ChunkPool) -> Chunk {
        if let Some(chunk) = pool.list.pop() {
            write_dispatch(
                &self.queue,
                &chunk.rectangle,
                0,
                chunk_to_rect(key, chunk_size),
            );
            chunk
        } else {
            let texture = create_chunk_texture(&self.device, chunk_size);
            let chunk = create_chunk(
                &self.device,
                &self.chunk_layout,
                texture,
                chunk_to_rect(key, chunk_size),
            );
            chunk
        }
    }
}

// --- Utils --- //

fn dispatch_workgroups_extend(cpass: &mut ComputePass, size: UVec2) {
    cpass.dispatch_workgroups(
        size.x.saturating_sub(1) / WORKGROUP_SIZE.x + 1,
        size.y.saturating_sub(1) / WORKGROUP_SIZE.y + 1,
        1,
    );
}

fn dispatch_workgroups_divide(cpass: &mut ComputePass, rects: &[Rectangle], div: u32) {
    let mut fnl = rects[0];
    for &rect in rects {
        if let Some(rect) = fnl.intersect(rect)
            && (rect.width() > 0 && rect.height() > 0)
        {
            fnl = rect;
        } else {
            return;
        }
    }

    dispatch_workgroups_extend(cpass, fnl.extend / div);
}

fn dispatch_workgroups(cpass: &mut ComputePass, rects: &[Rectangle]) {
    dispatch_workgroups_divide(cpass, rects, 1);
}

fn write_dispatch(queue: &Queue, buffer: &Buffer, index: u64, rect: Rectangle) {
    let uniform = DispatchUniform {
        coords: rect.origin.into(),
        size: rect.extend.into(),
    };
    queue.write_buffer(
        buffer,
        size_of::<DispatchUniform>() as u64 * index,
        bytes_of(&uniform),
    );
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

fn lower_chunk_of(chunk: ChunkKey) -> [ChunkKey; 4] {
    [
        (chunk.0 * 2, chunk.1 * 2, chunk.2 - 1),
        (chunk.0 * 2 + 1, chunk.1 * 2, chunk.2 - 1),
        (chunk.0 * 2, chunk.1 * 2 + 1, chunk.2 - 1),
        (chunk.0 * 2 + 1, chunk.1 * 2 + 1, chunk.2 - 1),
    ]
}

// --- Layouts --- //

const LAYOUT_DISPATCH: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_dispatch"),
    entries: &[BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: true,
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

/// Contains read storage texture of chunk in format `Rgba8Unorm` and chunk key in vec3
const LAYOUT_CHUNK_READ: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_chunk_read"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::ReadOnly,
                format: CHUNK_TEXTURE_FORMAT,
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

/// Contains write storage texture of chunk in format `Rgba8Unorm` and chunk key in vec3
const LAYOUT_CHUNK_WRITE: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_chunk_write"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: CHUNK_TEXTURE_FORMAT,
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

/// Contains write storage texture of chunk in format `Rgba8Unorm` and chunk key in vec3
const LAYOUT_CHUNK_READ_WRITE: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_chunk_write"),
    entries: &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::ReadWrite,
                format: CHUNK_TEXTURE_FORMAT,
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

/// Contains drawing rectangle and texture of chunk in format `Rgba8Unorm`
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

fn draws_dispatch_group(
    device: &Device,
    dispatch_draw_layout: &BindGroupLayout,
) -> (Buffer, Buffer, Buffer, BindGroup) {
    let dispatch = device.create_buffer(&BufferDescriptor {
        label: Some("layer_brush_dispatch"),
        size: size_of::<DispatchUniform>() as u64 * DISPATCH_CAPACITY,
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
        format: CHUNK_TEXTURE_FORMAT,
        usage: TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    })
}

fn create_chunk(
    device: &Device,
    chunk_layout: &ChunkLayout,
    texture: Texture,
    rect: Rectangle,
) -> Chunk {
    let rectangle = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("layer_chunk_buffer"),
        contents: bytes_of(&DispatchUniform {
            coords: rect.origin.into(),
            size: rect.extend.into(),
        }),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    let texture_fragment_view = texture.create_view(&TextureViewDescriptor {
        label: Some("layer_chunk_texture_view"),
        format: Some(CHUNK_TEXTURE_FORMAT),
        usage: Some(TextureUsages::TEXTURE_BINDING),
        ..Default::default()
    });

    let texture_compute_view = texture.create_view(&TextureViewDescriptor {
        label: Some("layer_chunk_texture_view"),
        format: Some(CHUNK_TEXTURE_FORMAT),
        usage: Some(TextureUsages::STORAGE_BINDING),
        ..Default::default()
    });

    let dispatch = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_dispatch"),
        layout: &chunk_layout.dispatch,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &rectangle,
                offset: 0,
                size: None,
            }),
        }],
    });

    let render = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_render"),
        layout: &chunk_layout.render,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_fragment_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &rectangle,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    let read = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_read"),
        layout: &chunk_layout.read,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_compute_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &rectangle,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    let write = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_write"),
        layout: &chunk_layout.write,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_compute_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &rectangle,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    let read_write = device.create_bind_group(&BindGroupDescriptor {
        label: Some("layer_chunk_read_write"),
        layout: &chunk_layout.read_write,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_compute_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &rectangle,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    Chunk {
        rectangle,
        dispatch,
        texture,
        render,
        read,
        write,
        read_write,
    }
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
                "{}{}{}",
                LIB_CAMERA,
                LIB_COLORSPACE,
                include_str!("layer/render.wgsl"),
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
        over: new_pipeline(
            BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer_chunk_over",
            "fs_main",
        ),
        over_fast: new_pipeline(
            BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer_chunk_over",
            "fs_fast",
        ),
        over_debug: new_pipeline(
            BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer_chunk_over_debug",
            "fs_debug0",
        ),
        replace: new_pipeline(BlendState::REPLACE, "layer_chunk_replace", "fs_main"),
        replace_fast: new_pipeline(BlendState::REPLACE, "layer_chunk_replace", "fs_fast"),
        replace_debug: new_pipeline(
            BlendState::REPLACE,
            "layer_chunk_replace_debug",
            "fs_debug1",
        ),
    }
}

fn mipmap_pipeline(device: &Device, chunk_layout: &ChunkLayout) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_mipmap"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                LIB_COLORSPACE,
                LIB_RECTANGLE,
                include_str!("layer/mipmap.wgsl"),
            )
            .into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_mipmap"),
        bind_group_layouts: &[
            Some(&chunk_layout.dispatch),
            Some(&chunk_layout.write),
            Some(&chunk_layout.read),
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
    read_write: bool,
    chunk_layout: &ChunkLayout,
) -> MergePipelines {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_merge"),
        bind_group_layouts: &match read_write {
            true => [
                Some(&chunk_layout.dispatch),
                Some(&chunk_layout.read_write),
                Some(&chunk_layout.read),
                Some(&chunk_layout.read_write),
            ],
            false => [
                Some(&chunk_layout.dispatch),
                Some(&chunk_layout.read),
                Some(&chunk_layout.read),
                Some(&chunk_layout.write),
            ],
        },
        immediate_size: 0,
    });

    let new_pipeline = |label, formula| {
        let constants = match read_write {
            true => [
                ("read", "read_write"),
                ("write", "read_write"),
                ("rectangle", LIB_RECTANGLE),
                ("composite", formula),
            ],
            false => [
                ("read", "read"),
                ("write", "write"),
                ("rectangle", LIB_RECTANGLE),
                ("composite", formula),
            ],
        };

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(
                shader_compile(include_str!("layer/merge.wgsl"), &constants[..]).into(),
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

    MergePipelines {
        over: new_pipeline("merge_over", "src + dst * (1 - src.a)"),
        replace: new_pipeline("merge_replace", "src"),
        erase: new_pipeline("merge_erase", "dst * (1 - src.a)"),
    }
}

fn copy_pipeline(device: &Device, chunk_layout: &ChunkLayout) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_copy"),
        source: ShaderSource::Wgsl(
            format!("{}{}", LIB_RECTANGLE, include_str!("layer/copy.wgsl")).into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_copy"),
        bind_group_layouts: &[
            Some(&chunk_layout.dispatch),
            Some(&chunk_layout.write),
            Some(&chunk_layout.read),
        ],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("layer_copy"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn clear_pipeline(device: &Device, chunk_layout: &ChunkLayout) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("layer_clear"),
        source: ShaderSource::Wgsl(
            format!("{}{}", LIB_RECTANGLE, include_str!("layer/clear.wgsl")).into(),
        ),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_clear"),
        bind_group_layouts: &[Some(&chunk_layout.dispatch), Some(&chunk_layout.write)],
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

fn brush_pipelines(
    device: &Device,
    read_write: bool,
    dispatch_draw_layout: &BindGroupLayout,
    chunk_layout: &ChunkLayout,
) -> BrushPipelines {
    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &match read_write {
            true => [
                Some(dispatch_draw_layout),
                Some(&chunk_layout.read_write),
                Some(&chunk_layout.read_write),
            ],
            false => [
                Some(dispatch_draw_layout),
                Some(&chunk_layout.read),
                Some(&chunk_layout.write),
            ],
        },
        immediate_size: 0,
    });

    let bridge_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[
            Some(dispatch_draw_layout),
            Some(&chunk_layout.read),
            Some(&chunk_layout.write),
        ],
        immediate_size: 0,
    });

    let round_pipeline = |label, formula| {
        let constants = match read_write {
            true => [
                ("read", "read_write"),
                ("write", "read_write"),
                ("rectangle", LIB_RECTANGLE),
                ("composite", formula),
            ],
            false => [
                ("read", "read"),
                ("write", "write"),
                ("rectangle", LIB_RECTANGLE),
                ("composite", formula),
            ],
        };

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(
                shader_compile(include_str!("layer/brush/round.wgsl"), &constants[..]).into(),
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
        // bridge mode does not need read_write bind
        let constants = [
            ("read", "read"),
            ("write", "write"),
            ("constant", LIB_CONSTANT),
            ("rectangle", LIB_RECTANGLE),
        ];

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(
                shader_compile(include_str!("layer/brush/blur.wgsl"), &constants).into(),
            ),
        });
        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&bridge_layout),
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
