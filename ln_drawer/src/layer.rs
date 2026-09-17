pub mod brush;
pub mod input;
pub mod stream;
pub mod traveler;
pub mod wrapper;

use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bytemuck::bytes_of;
use glam::{IVec2, UVec2, Vec4};
use hashbrown::HashMap;
use palette::Srgba;
use wgpu::{
    Adapter, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, FilterMode, FragmentState, MapMode,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPass, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureFormatFeatureFlags,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    WasmNotSend,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    widgets::shaders::shader_compile,
};

pub type ChunkKey = (i32, i32, u8);

pub const DEFAULT_MIPMAP_DISABLED: u8 = 1;
pub const DEFAULT_MIPMAP_ENABLED: u8 = 8;
pub const DEFAULT_CHUNK_SIZE: u32 = 512;

pub const CHUNK_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

const DISPATCH_CAPACITY: u64 = 4;
const DRAWS_ARRAY_CAPACITY: u64 = 0x2000;
const DRAWS_STATE_CAPACITY: u64 = 0x2000;
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
    draws_length: Buffer,
    draws_array: Buffer,
    draws_state: Buffer,
    draws_dispatch_group: BindGroup,

    readback_sample: Buffer,
    readback_share: Buffer,
    readback_group: BindGroup,
    readback_mapped: Arc<AtomicBool>,

    render_pipelines: RenderPipelines,
    merge_pipelines: MergePipelines,
    mipmap_pipeline: ComputePipeline,
    copy_pipeline: ComputePipeline,
    readback_pipeline: ComputePipeline,
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

pub struct Standalone {
    pub chunk: Chunk,
    pub rect: Rectangle,
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
struct ChunkLayout {
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
    smudge: ComputePipeline,
    smudge_prepare_stroke: ComputePipeline,
    smudge_prepare_draw: ComputePipeline,
    tint: ComputePipeline,
    round_over: ComputePipeline,
    round_erase: ComputePipeline,
    pixel_over: ComputePipeline,
    pixel_erase: ComputePipeline,
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
        let (draws_dispatch, draws_length, draws_array, draws_state, draws_dispatch_group) =
            draws_dispatch_group(&device, &draw_dispatch_layout);

        let color_readback_layout = device.create_bind_group_layout(&LAYOUT_COLOR_READBACK);
        let (readback_sample, readback_share, readback_group) =
            color_readback_group(&device, &color_readback_layout, &draws_state);

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
        let readback_pipeline = readback_pipeline(&device, &color_readback_layout, &chunk_layout);
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
            draws_length,
            draws_array,
            draws_dispatch_group,
            draws_state,
            readback_sample,
            readback_share,
            readback_group,
            readback_mapped: Arc::new(AtomicBool::new(false)),
            render_pipelines,
            merge_pipelines,
            mipmap_pipeline,
            copy_pipeline,
            readback_pipeline,
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

    /// Assume `self.controlled` is false.
    pub fn layer_prepare_rect(
        &self,
        layer: &mut Layer,
        pool: &mut ChunkPool,
        rect: Rectangle,
    ) -> Vec<ChunkKey> {
        assert!(!layer.controlled, "controlled layer cannot prepare chunks");
        assert_eq!(
            layer.chunk_size, pool.chunk_size,
            "pool chunk_size does not matched"
        );

        let chunks = layer.get_missing_chunks(rect);
        for &key in &chunks {
            let dst_chunk = pool.pop(key, layer.chunk_size, self);
            layer.chunks.insert(key, dst_chunk);
        }

        return chunks;
    }

    /// Unloaded chunk will be ignored.
    fn layer_copy_chunk(
        &self,
        src: &Layer,
        dst: &mut Layer,
        cpass: &mut ComputePass<'_>,
        key: ChunkKey,
    ) -> bool {
        assert_eq!(
            src.chunk_size, dst.chunk_size,
            "reference layer chunk_size does not matched"
        );

        let src_chunk = src.chunks.get(&key);
        let dst_chunk = dst.chunks.get(&key);

        if let (Some(src_chunk), Some(dst_chunk)) = (src_chunk, dst_chunk) {
            cpass.set_pipeline(&self.copy_pipeline);
            cpass.set_bind_group(0, &dst_chunk.dispatch, &[0]);
            cpass.set_bind_group(1, &dst_chunk.write, &[]);
            cpass.set_bind_group(2, &src_chunk.read, &[]);
            let chunk_rect = chunk_to_rect(key, dst.chunk_size);
            dispatch_workgroups(cpass, &[chunk_rect]);
            true
        } else {
            false
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

    pub fn pick_color(
        &self,
        layer: &Layer,
        point: IVec2,
        f: impl FnOnce(Srgba) + WasmNotSend + 'static,
    ) {
        let mapped = self.readback_mapped.load(Ordering::Acquire);
        if mapped {
            return;
        }

        self.queue
            .write_buffer(&self.readback_sample, 0, bytes_of(&point));

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_color_readback"),
            });
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_color_readback"),
            timestamp_writes: None,
        });

        let chunk_key = point_to_chunks(point, 0, layer.chunk_size);

        let Some(chunk) = layer.chunks.get(&(chunk_key.0, chunk_key.1, 0)) else {
            return;
        };

        cpass.set_pipeline(&self.readback_pipeline);
        cpass.set_bind_group(0, Some(&self.readback_group), &[]);
        cpass.set_bind_group(1, Some(&chunk.read), &[]);
        cpass.dispatch_workgroups(1, 1, 1);

        drop(cpass);

        encoder.copy_buffer_to_buffer(
            &self.draws_state,
            0,
            &self.readback_share,
            0,
            size_of::<Vec4>() as u64,
        );

        self.queue.submit([encoder.finish()]);

        let readback_share = self.readback_share.clone();
        let readback_mapped = self.readback_mapped.clone();
        self.readback_mapped.store(true, Ordering::Release);
        self.readback_share
            .map_async(MapMode::Read, .., move |state| {
                if state.is_err() {
                    return;
                }
                let view = readback_share.get_mapped_range(..).unwrap();
                let color = Srgba::from(
                    *bytemuck::cast_slice::<_, f32>(&view[..])
                        .as_array::<4>()
                        .unwrap(),
                );
                f(color.into_format());
                drop(view);
                readback_mapped.store(false, Ordering::Release);
                readback_share.unmap();
            });
    }
}

impl Layer {
    fn get_missing_chunks(&self, rect: Rectangle) -> Vec<ChunkKey> {
        let mut missing = Vec::new();
        for mipmap in 0..self.mipmap_levels {
            let (start, end) = rect_to_chunks(rect, mipmap, self.chunk_size);
            for chunk_x in start.0..end.0 {
                for chunk_y in start.1..end.1 {
                    let key = (chunk_x, chunk_y, mipmap);
                    if !self.chunks.contains_key(&key) {
                        missing.push(key);
                    }
                }
            }
        }
        missing
    }
}

impl ChunkPool {
    fn pop(&mut self, key: ChunkKey, chunk_size: u32, pipeline: &LayerPipeline) -> Chunk {
        if let Some(chunk) = self.list.pop() {
            write_dispatch(
                &pipeline.queue,
                &chunk.rectangle,
                0,
                chunk_to_rect(key, chunk_size),
            );
            chunk
        } else {
            let texture = create_chunk_texture(&pipeline.device, chunk_size);
            let chunk = create_chunk(
                &pipeline.device,
                &pipeline.chunk_layout,
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
    queue.write_buffer(
        buffer,
        size_of::<Rectangle>() as u64 * index,
        bytes_of(&rect),
    );
}

fn chunk_to_rect((x, y, z): ChunkKey, chunk_size: u32) -> Rectangle {
    let size = chunk_size << z;
    Rectangle {
        origin: IVec2::new(x, y) * size as i32,
        extend: UVec2::splat(size),
    }
}

fn point_to_chunks(point: IVec2, mipmap: u8, chunk_size: u32) -> (i32, i32) {
    let size = (chunk_size << mipmap) as i32;
    let chunk_src = (point.x.div_euclid(size), point.y.div_euclid(size));
    chunk_src
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
        BindGroupLayoutEntry {
            binding: 3,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
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

const LAYOUT_COLOR_READBACK: BindGroupLayoutDescriptor = BindGroupLayoutDescriptor {
    label: Some("color_readback"),
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
                ty: BufferBindingType::Storage { read_only: false },
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
        size: size_of::<Rectangle>() as u64,
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
) -> (Buffer, Buffer, Buffer, Buffer, BindGroup) {
    let dispatch = device.create_buffer(&BufferDescriptor {
        label: Some("layer_brush_dispatch"),
        size: size_of::<Rectangle>() as u64 * DISPATCH_CAPACITY,
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

    let draws_state = device.create_buffer(&BufferDescriptor {
        label: Some("layer_storage"),
        size: DRAWS_STATE_CAPACITY,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
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
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &draws_state,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    (
        dispatch,
        draws_length,
        draws_array,
        draws_state,
        dispatch_group_draw,
    )
}

fn color_readback_group(
    device: &Device,
    color_readback_layout: &BindGroupLayout,
    drwas_state: &Buffer,
) -> (Buffer, Buffer, BindGroup) {
    let sample_position = device.create_buffer(&BufferDescriptor {
        label: Some("color_readback_sample_position"),
        size: size_of::<IVec2>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let share_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("color_readback_share_buffer"),
        size: size_of::<Vec4>() as u64,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let readback_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("color_readback"),
        layout: color_readback_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &sample_position,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: drwas_state,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    (sample_position, share_buffer, readback_group)
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
        contents: bytes_of(&rect),
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
        source: ShaderSource::Wgsl(shader_compile(include_str!("layer/render.wgsl"), &[]).into()),
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
        source: ShaderSource::Wgsl(shader_compile(include_str!("layer/mipmap.wgsl"), &[]).into()),
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
                ("composite", formula),
            ],
            false => [("read", "read"), ("write", "write"), ("composite", formula)],
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
        source: ShaderSource::Wgsl(shader_compile(include_str!("layer/copy.wgsl"), &[]).into()),
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

fn readback_pipeline(
    device: &Device,
    color_readback_layout: &BindGroupLayout,
    chunk_layout: &ChunkLayout,
) -> ComputePipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("color_readback"),
        source: ShaderSource::Wgsl(shader_compile(include_str!("layer/readback.wgsl"), &[]).into()),
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("color_readback"),
        bind_group_layouts: &[Some(color_readback_layout), Some(&chunk_layout.read)],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("color_readback"),
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
        source: ShaderSource::Wgsl(shader_compile(include_str!("layer/clear.wgsl"), &[]).into()),
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

    let layout_prepare_stroke = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[Some(dispatch_draw_layout)],
        immediate_size: 0,
    });

    let layout_prepare_draw = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[Some(dispatch_draw_layout), Some(&chunk_layout.read)],
        immediate_size: 0,
    });

    let layout_bridge = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[
            Some(dispatch_draw_layout),
            Some(&chunk_layout.read),
            Some(&chunk_layout.write),
        ],
        immediate_size: 0,
    });

    let constants = match read_write {
        true => [("read", "read_write"), ("write", "read_write")],
        false => [("read", "read"), ("write", "write")],
    };

    // bridge mode does not need read_write bind
    let constants_bridge = [("read", "read"), ("write", "write")];

    const COMPOSITE_OVER: &str = "src + dst * (1 - src.a)";
    const COMPOSITE_ERASE: &str = "dst * (1 - src.a)";
    BrushPipelines {
        blur: general_brush_pipeline(
            device,
            &layout_bridge,
            "blur",
            include_str!("layer/brush/blur.wgsl"),
            &constants_bridge,
            "",
            "cs_main",
        ),
        smudge: general_brush_pipeline(
            device,
            &layout_bridge,
            "smudge",
            include_str!("layer/brush/smudge.wgsl"),
            &constants_bridge,
            "",
            "cs_main",
        ),
        smudge_prepare_draw: general_brush_pipeline(
            device,
            &layout_prepare_draw,
            "smudge_prepare_draw",
            include_str!("layer/brush/smudge_prepare_draw.wgsl"),
            &constants_bridge,
            "",
            "cs_main",
        ),
        smudge_prepare_stroke: general_brush_pipeline(
            device,
            &layout_prepare_stroke,
            "smudge_prepare_stroke",
            include_str!("layer/brush/smudge_prepare_stroke.wgsl"),
            &constants_bridge,
            "",
            "cs_main",
        ),
        tint: general_brush_pipeline(
            device,
            &layout,
            "tint",
            include_str!("layer/brush/tint.wgsl"),
            &constants,
            "",
            "cs_main",
        ),
        round_over: general_brush_pipeline(
            device,
            &layout,
            "round_over",
            include_str!("layer/brush/round.wgsl"),
            &constants[..],
            COMPOSITE_OVER,
            "cs_main",
        ),
        round_erase: general_brush_pipeline(
            device,
            &layout,
            "round_erase",
            include_str!("layer/brush/round.wgsl"),
            &constants[..],
            COMPOSITE_ERASE,
            "cs_main",
        ),
        pixel_over: general_brush_pipeline(
            device,
            &layout,
            "pixel_over",
            include_str!("layer/brush/pixel.wgsl"),
            &constants[..],
            COMPOSITE_OVER,
            "cs_main",
        ),
        pixel_erase: general_brush_pipeline(
            device,
            &layout,
            "pixel_erase",
            include_str!("layer/brush/pixel.wgsl"),
            &constants[..],
            COMPOSITE_ERASE,
            "cs_main",
        ),
    }
}

fn general_brush_pipeline(
    device: &Device,
    layout: &wgpu::PipelineLayout,
    label: &str,
    shader: &str,
    constants: &[(&str, &str)],
    composite: &str,
    entry: &str,
) -> ComputePipeline {
    let maps = &[constants, &[("composite", composite)][..]].concat()[..];
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(label),
        source: ShaderSource::Wgsl(shader_compile(shader, maps).into()),
    });
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        module: &shader,
        entry_point: Some(entry),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
}
