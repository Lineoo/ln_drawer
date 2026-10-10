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
        mpsc::Sender,
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
    PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PrimitiveState,
    PrimitiveTopology, Queue, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureFormatFeatureFlags,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    WasmNotSend,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    layer::{brush::Brush, stream::ThreadInput},
    measures::{FI64Ext, Rectangle},
    widgets::shaders::shader_compile,
};

pub const DEFAULT_MIPMAP_CAP: u8 = 8;
pub const DEFAULT_CHUNK_SIZE: UVec2 = UVec2::splat(512);

pub const CHUNK_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

const DISPATCH_CAPACITY: u64 = 4;
const DRAWS_ARRAY_CAPACITY: u64 = 0x2000;
const DRAWS_STATE_CAPACITY: u64 = 0x2000;
const WORKGROUP_SIZE: UVec2 = UVec2::new(16, 16);

// function: render, merge, mipmap, clear & chunk recycle
pub struct LayerPipeline {
    pub _adapter: Adapter,
    pub device: Device,
    pub queue: Queue,

    chunk_layout: ChunkLayout,

    work_rect: Buffer,
    work_dispatch_group: BindGroup,

    sampler_group_unfiltered: BindGroup,
    sampler_group_filtered: BindGroup,

    draws_rect: Buffer,
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
    brush_pipelines: BrushPipelines,
    mipmap_pipeline: ComputePipeline,
    copy_pipeline: ComputePipeline,
    readback_pipeline: ComputePipeline,
    clear_pipeline: ComputePipeline,
}

#[derive(Clone)]
pub struct Chunk {
    // TODO dynamic offset
    pub rectangle: Buffer,
    pub texture: Texture,
    pub dispatch: BindGroup,
    pub render: BindGroup,
    pub read: BindGroup,
    pub write: BindGroup,
    pub read_write: Option<BindGroup>,
}

#[derive(Clone)]
struct ChunkLayout {
    dispatch: BindGroupLayout,
    render: BindGroupLayout,
    read: BindGroupLayout,
    write: BindGroupLayout,
    read_write: Option<BindGroupLayout>,
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

        let mut support_read_write =
            (texture_features.flags).contains(TextureFormatFeatureFlags::STORAGE_READ_WRITE);

        log::debug!("device storage read-write: {support_read_write}");

        if std::env::var_os("LNDRAWER_NO_READ_WRITE").is_some() {
            log::warn!("storage read-write disabled by LNDRAWER_NO_READ_WRITE");
            support_read_write = false;
        }

        let dispatch_layout = device.create_bind_group_layout(&LAYOUT_WORK_DISPATCH);
        let (work_rect, work_dispatch_group) = work_dispatch_group(&device, &dispatch_layout);

        let sampler_layout = device.create_bind_group_layout(&LAYOUT_SAMPLER);
        let (sampler_group_unfiltered, sampler_group_filtered) =
            sampler_groups(&device, &sampler_layout);

        let draw_dispatch_layout = device.create_bind_group_layout(&LAYOUT_DRAW_DISPATCH);
        let (draws_rect, draws_length, draws_array, draws_state, draws_dispatch_group) =
            draws_dispatch_group(&device, &draw_dispatch_layout);

        let color_readback_layout = device.create_bind_group_layout(&LAYOUT_COLOR_READBACK);
        let (readback_sample, readback_share, readback_group) =
            color_readback_group(&device, &color_readback_layout, &draws_state);

        let chunk_layout = ChunkLayout {
            dispatch: dispatch_layout,
            render: device.create_bind_group_layout(&LAYOUT_CHUNK_RENDER),
            read: device.create_bind_group_layout(&LAYOUT_CHUNK_READ),
            write: device.create_bind_group_layout(&LAYOUT_CHUNK_WRITE),
            read_write: support_read_write
                .then(|| device.create_bind_group_layout(&LAYOUT_CHUNK_READ_WRITE)),
        };

        let render_pipelines = render_pipelines(
            &device,
            surface_format,
            camera_bind_layout,
            sampler_layout,
            &chunk_layout.render,
        );

        let brush_pipelines = brush_pipelines(&device, &draw_dispatch_layout, &chunk_layout);
        let merge_pipelines = merge_pipelines(&device, &chunk_layout);
        let mipmap_pipeline = mipmap_pipeline(&device, &chunk_layout);
        let copy_pipeline = copy_pipeline(&device, &chunk_layout);
        let readback_pipeline = readback_pipeline(&device, &color_readback_layout, &chunk_layout);
        let clear_pipeline = clear_pipeline(&device, &chunk_layout);

        LayerPipeline {
            _adapter,
            device,
            queue,
            chunk_layout,
            work_rect,
            work_dispatch_group,
            sampler_group_unfiltered,
            sampler_group_filtered,
            draws_rect,
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
        }
    }

    pub fn create_chunk_texture(&self, size: UVec2) -> Texture {
        create_chunk_texture(&self.device, size)
    }

    pub fn create_chunk(&self, texture: Texture, rect: Rectangle) -> Chunk {
        create_chunk(&self.device, &self.chunk_layout, texture, rect)
    }

    pub fn create_empty_chunk(&self, size: UVec2, rect: Rectangle) -> Chunk {
        let texture = self.create_chunk_texture(size);
        self.create_chunk(texture, rect)
    }

    pub fn create_standalone(&self, rect: Rectangle) -> Standalone {
        let chunk = self.create_empty_chunk(rect.extend, rect);
        Standalone { chunk, rect }
    }

    pub fn raw_remove_associated(
        &self,
        cpass: &mut ComputePass,
        raw: &mut RawLayer,
        cache: &CacheLayers,
        pool: &mut ChunkPool,
        dirty: Rectangle,
    ) {
        for chunk_key in rect_chunks_iter(dirty, raw.chunk_size) {
            let chunk_rect = chunks_key_to_rect(chunk_key, raw.chunk_size);

            let Some(cache_chunk) = cache.chunks.get(&(chunk_key.0, chunk_key.1, 0)) else {
                log::error!("cache not loaded");
                continue;
            };
            let Some(raw_chunk) = raw.chunks.get(&chunk_key) else {
                log::error!("raw not loaded");
                continue;
            };

            match (cache_chunk, raw_chunk) {
                (CacheLayerChunk::Filled(cache_chunk), RawLayerChunk::Associated) => {
                    let raw_chunk = pool.create_empty_chunk(
                        self,
                        raw.chunk_size,
                        chunks_key_to_rect(chunk_key, raw.chunk_size),
                    );
                    self.copy_chunk_direct(cpass, cache_chunk, chunk_rect, &raw_chunk, chunk_rect);
                    raw.swap_chunk(chunk_key, RawLayerChunk::Filled(raw_chunk));
                }
                (CacheLayerChunk::Transparent, RawLayerChunk::Associated) => {
                    raw.swap_chunk(chunk_key, RawLayerChunk::Transparent);
                }
                (
                    CacheLayerChunk::Filled(_) | CacheLayerChunk::Transparent,
                    RawLayerChunk::AssociatedCached(raw_chunk),
                ) => {
                    raw.swap_chunk(chunk_key, RawLayerChunk::Filled(raw_chunk.clone()));
                }
                (CacheLayerChunk::Filled(_), RawLayerChunk::Transparent) => {}
                (CacheLayerChunk::Filled(_), RawLayerChunk::Filled(_)) => {}
                (CacheLayerChunk::Transparent, RawLayerChunk::Transparent) => {}
                (CacheLayerChunk::Transparent, RawLayerChunk::Filled(_)) => {}
            }
        }
    }

    pub fn cache_mipmaps(
        &self,
        cpass: &mut ComputePass,
        cache: &mut CacheLayers,
        pool: &mut ChunkPool,
        dirty: Rectangle,
    ) {
        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        for lower_key in rect_chunks_stack_iter(dirty, cache.chunk_size, cache.mipmap_cap - 1) {
            let upper_key = upper_chunk_of(lower_key);
            let upper_rect = chunks_stack_key_to_rect(upper_key, cache.chunk_size);
            let lower_rect = chunks_stack_key_to_rect(lower_key, cache.chunk_size);
            let scale = 1u32 << upper_key.2;

            let Some(lower_chunk) = cache.chunks.get(&lower_key) else {
                log::error!("lower chunk not loaded");
                continue;
            };
            let Some(upper_chunk) = cache.chunks.get(&upper_key) else {
                log::error!("upper chunk not loaded");
                continue;
            };

            match (lower_chunk, upper_chunk) {
                (CacheLayerChunk::Filled(lower_chunk), CacheLayerChunk::Filled(upper_chunk)) => {
                    cpass.set_bind_group(1, Some(&upper_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&lower_chunk.read), &[]);
                    dispatch_workgroups_divide(cpass, &[dirty, upper_rect, lower_rect], scale);
                }
                (CacheLayerChunk::Filled(lower_chunk), CacheLayerChunk::Transparent) => {
                    let upper_chunk = pool.create_empty_chunk(
                        self,
                        cache.chunk_size,
                        chunks_stack_key_to_rect(upper_key, cache.chunk_size),
                    );
                    cpass.set_bind_group(1, Some(&upper_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&lower_chunk.read), &[]);
                    dispatch_workgroups_divide(cpass, &[dirty, upper_rect, lower_rect], scale);
                    cache.swap_chunk(upper_key, CacheLayerChunk::Filled(upper_chunk));
                }
                (CacheLayerChunk::Transparent, CacheLayerChunk::Filled(upper_chunk)) => {
                    let lower_chunk = pool.create_empty_chunk(
                        self,
                        cache.chunk_size,
                        chunks_stack_key_to_rect(lower_key, cache.chunk_size),
                    );
                    cpass.set_bind_group(1, Some(&upper_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&lower_chunk.read), &[]);
                    dispatch_workgroups_divide(cpass, &[dirty, upper_rect, lower_rect], scale);
                    cache.swap_chunk(lower_key, CacheLayerChunk::Filled(lower_chunk));
                }
                (CacheLayerChunk::Transparent, CacheLayerChunk::Transparent) => {}
            }
        }
    }

    pub fn pick_color(
        &self,
        raw: &RawLayer,
        cache: &CacheLayers,
        point: IVec2,
        f: impl FnOnce(Srgba) + WasmNotSend + 'static,
    ) {
        let mapped = self.readback_mapped.load(Ordering::Acquire);
        if mapped {
            return;
        }

        let chunk_key = point_to_chunks(point, raw.chunk_size);
        let chunk = match raw.chunks.get(&chunk_key) {
            Some(RawLayerChunk::Filled(chunk)) => chunk,
            Some(RawLayerChunk::AssociatedCached(_)) | Some(RawLayerChunk::Associated) => {
                match cache.chunks.get(&(chunk_key.0, chunk_key.1, 0)) {
                    Some(CacheLayerChunk::Filled(chunk)) => chunk,
                    Some(CacheLayerChunk::Transparent) => {
                        f(Srgba::new(0.0, 0.0, 0.0, 0.0));
                        return;
                    }
                    None => {
                        return;
                    }
                }
            }
            Some(RawLayerChunk::Transparent) => {
                f(Srgba::new(0.0, 0.0, 0.0, 0.0));
                return;
            }
            None => {
                return;
            }
        };

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

    pub fn copy_chunk_direct(
        &self,
        cpass: &mut ComputePass,
        src: &Chunk,
        src_rect: Rectangle,
        dst: &Chunk,
        dst_rect: Rectangle,
    ) {
        cpass.set_pipeline(&self.copy_pipeline);
        cpass.set_bind_group(0, Some(&dst.dispatch), &[0]);
        cpass.set_bind_group(1, Some(&dst.write), &[]);
        cpass.set_bind_group(2, Some(&src.read), &[]);
        dispatch_workgroups(cpass, &[src_rect, dst_rect]);
    }

    /// use `dispatch_group`
    pub fn copy_chunk(
        &self,
        cpass: &mut ComputePass<'_>,
        src_chunk: &Chunk,
        src_rect: Rectangle,
        dst_chunk: &Chunk,
        dst_rect: Rectangle,
        dispatch: Rectangle,
    ) {
        cpass.set_pipeline(&self.copy_pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
        cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
        dispatch_workgroups(cpass, &[dispatch, src_rect, dst_rect]);
    }

    pub fn draw_chunk_cross<T: Brush + ?Sized>(
        &self,
        cpass: &mut ComputePass,
        src: &Chunk,
        src_rect: Rectangle,
        dst: &Chunk,
        dst_rect: Rectangle,
        brush: &T,
        dispatch: Rectangle,
    ) {
        brush.set_pipeline(cpass, self, WriteMode::OutOfPlace);
        cpass.set_bind_group(0, Some(&self.draws_dispatch_group), &[]);
        cpass.set_bind_group(1, Some(&src.read), &[]);
        cpass.set_bind_group(2, Some(&dst.write), &[]);
        dispatch_workgroups(cpass, &[dispatch, src_rect, dst_rect]);
    }

    pub fn draw_chunk_swap<T: Brush + ?Sized>(
        &self,
        cpass: &mut ComputePass,
        chunk: &Chunk,
        chunk_rect: Rectangle,
        swap: &Chunk,
        swap_rect: Rectangle,
        brush: &T,
        dispatch: Rectangle,
    ) {
        brush.set_pipeline(cpass, self, WriteMode::OutOfPlace);
        cpass.set_bind_group(0, Some(&self.draws_dispatch_group), &[]);
        cpass.set_bind_group(1, Some(&chunk.read), &[]);
        cpass.set_bind_group(2, Some(&swap.write), &[]);
        dispatch_workgroups(cpass, &[dispatch, chunk_rect, swap_rect]);

        cpass.set_pipeline(&self.copy_pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        cpass.set_bind_group(1, Some(&chunk.write), &[]);
        cpass.set_bind_group(2, Some(&swap.read), &[]);
        dispatch_workgroups(cpass, &[dispatch, chunk_rect, swap_rect]);
    }

    pub fn draw_chunk_read_write<T: Brush + ?Sized>(
        &self,
        cpass: &mut ComputePass,
        chunk: &Chunk,
        chunk_rect: Rectangle,
        brush: &T,
        dispatch: Rectangle,
    ) {
        brush.set_pipeline(cpass, self, WriteMode::InPlace);
        cpass.set_bind_group(0, Some(&self.draws_dispatch_group), &[]);
        cpass.set_bind_group(1, Some(chunk.read_write.as_ref().unwrap()), &[]);
        cpass.set_bind_group(2, Some(chunk.read_write.as_ref().unwrap()), &[]);
        dispatch_workgroups(cpass, &[dispatch, chunk_rect]);
    }

    pub fn draw_chunk<T: Brush + ?Sized>(
        &self,
        cpass: &mut ComputePass,
        chunk: &Chunk,
        chunk_rect: Rectangle,
        swap: Option<&Chunk>,
        swap_rect: Option<Rectangle>,
        brush: &T,
        dispatch: Rectangle,
    ) {
        if let Some((swap, swap_rect)) = swap.zip(swap_rect) {
            self.draw_chunk_swap(cpass, chunk, chunk_rect, swap, swap_rect, brush, dispatch)
        } else {
            self.draw_chunk_read_write(cpass, chunk, chunk_rect, brush, dispatch)
        }
    }

    /// use `work_dispatch_group`
    pub fn merge_chunk_over_cross(
        &self,
        cpass: &mut ComputePass,
        src: &Chunk,
        src_rect: Rectangle,
        dst: &Chunk,
        dst_rect: Rectangle,
        swp: &Chunk,
        swp_rect: Rectangle,
        dispatch: Rectangle,
    ) {
        let pipeline = self
            .merge_pipelines
            .get(WriteMode::OutOfPlace, CompositeMode::Over);
        cpass.set_pipeline(pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        cpass.set_bind_group(1, Some(&dst.read), &[]);
        cpass.set_bind_group(2, Some(&src.read), &[]);
        cpass.set_bind_group(3, Some(&swp.write), &[]);
        dispatch_workgroups(cpass, &[dispatch, src_rect, dst_rect, swp_rect]);
    }

    /// use `work_dispatch_group`
    #[expect(unused)]
    pub fn merge_chunk_over_read_write(
        &self,
        cpass: &mut ComputePass,
        src: &Chunk,
        src_rect: Rectangle,
        dst: &Chunk,
        dst_rect: Rectangle,
        dispatch: Rectangle,
    ) {
        let pipeline = self
            .merge_pipelines
            .get(WriteMode::InPlace, CompositeMode::Over);
        cpass.set_pipeline(pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        cpass.set_bind_group(1, Some(dst.read_write.as_ref().unwrap()), &[]);
        cpass.set_bind_group(2, Some(&src.read), &[]);
        cpass.set_bind_group(3, Some(dst.read_write.as_ref().unwrap()), &[]);
        dispatch_workgroups(cpass, &[dispatch, src_rect, dst_rect]);
    }

    /// use `work_dispatch_group`
    pub fn clear_chunk(
        &self,
        cpass: &mut ComputePass,
        chunk: &Chunk,
        chunk_rect: Rectangle,
        dispatch: Rectangle,
    ) {
        cpass.set_pipeline(&self.clear_pipeline);
        cpass.set_bind_group(0, Some(&self.work_dispatch_group), &[0]);
        cpass.set_bind_group(1, Some(&chunk.write), &[]);
        dispatch_workgroups(cpass, &[dispatch, chunk_rect]);
    }

    pub fn clear_chunk_direct(
        &self,
        cpass: &mut ComputePass,
        chunk: &Chunk,
        chunk_rect: Rectangle,
    ) {
        cpass.set_pipeline(&self.clear_pipeline);
        cpass.set_bind_group(0, Some(&chunk.dispatch), &[0]);
        cpass.set_bind_group(1, Some(&chunk.write), &[]);
        dispatch_workgroups(cpass, &[chunk_rect]);
    }
}

// ---- collections of chunk ---- //

/// layer for scratch, hashmap existence stands for whether it's non-transparent.
pub struct PlainLayer {
    pub chunks: HashMap<(i32, i32), Chunk>,
    pub chunk_size: UVec2,
}

/// layer for rendering, hashmap existence stands for whether it's loaded or not.
pub struct CacheLayers {
    pub chunks: HashMap<(i32, i32, u8), CacheLayerChunk>,
    pub chunk_size: UVec2,
    pub mipmap_cap: u8,
    pub thread_tx: Sender<ThreadInput>,
}

#[derive(Clone)]
pub enum CacheLayerChunk {
    Transparent,
    Filled(Chunk),
}

impl CacheLayers {
    pub fn swap_chunk(&mut self, key: (i32, i32, u8), chunk: CacheLayerChunk) {
        self.chunks.insert(key, chunk.clone());
        self.thread_tx
            .send(ThreadInput::SwapCache(key, chunk))
            .unwrap();
    }
}

/// layer of raw data, `Associated` variety means chunk's data equals to its correlated cached
/// layer level 0 chunk, hashmap existence stands for whether it's loaded or not.
pub struct RawLayer {
    pub chunks: HashMap<(i32, i32), RawLayerChunk>,
    pub chunk_size: UVec2,
    pub thread_tx: Sender<ThreadInput>,
}

#[derive(Clone)]
pub enum RawLayerChunk {
    Transparent,
    Associated,
    AssociatedCached(Chunk),
    Filled(Chunk),
}

impl RawLayer {
    pub fn swap_chunk(&mut self, key: (i32, i32), chunk: RawLayerChunk) {
        self.chunks.insert(key, chunk.clone());
        self.thread_tx
            .send(ThreadInput::SwapRaw(key, chunk))
            .unwrap();
    }
}

pub struct Standalone {
    pub chunk: Chunk,
    pub rect: Rectangle,
}

pub struct ChunkPool {
    /// chunks here are directly available in full transparent pixels
    pub list: Vec<Chunk>,
    /// chunks here are waiting for queue to refresh its rectangle buffer
    pub stage: Vec<Chunk>,
    pub chunk_size: UVec2,
}

impl ChunkPool {
    fn create_empty_chunk(
        &mut self,
        lp: &LayerPipeline,
        chunk_size: UVec2,
        rect: Rectangle,
    ) -> Chunk {
        if chunk_size != self.chunk_size {
            log::warn!(
                "wrong chunk pool ({}x) are being called (expect {}x)",
                self.chunk_size,
                chunk_size
            );
            lp.create_empty_chunk(chunk_size, rect)
        } else if let Some(chunk) = self.list.pop() {
            write_dispatch(&lp.queue, &chunk.rectangle, 0, rect);
            chunk
        } else {
            lp.create_empty_chunk(chunk_size, rect)
        }
    }

    fn flip(&mut self) {
        self.list.append(&mut self.stage);
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

// ---- ChunkKey Utils ---- //

fn point_to_chunks(point: IVec2, chunk_size: UVec2) -> (i32, i32) {
    let size = chunk_size.as_ivec2();
    let chunk = point.div_euclid(size);
    chunk.into()
}

fn point_zoom_to_chunks_stack(point: IVec2, zoom: i64, chunk_size: UVec2) -> (i32, i32, u8) {
    let mipmap = zoom_to_mipmap(zoom);
    let size = (chunk_size << mipmap).as_ivec2();
    let chunk = point.div_euclid(size);
    (chunk.x, chunk.y, mipmap)
}

fn zoom_to_mipmap(zoom: i64) -> u8 {
    (-zoom).q32_round().max(0) as u8
}

fn rect_chunks_iter(rect: Rectangle, chunk_size: UVec2) -> impl Iterator<Item = (i32, i32)> {
    let (chunk_src, chunk_dst) = rect_chunks_stack_layer_range(rect, chunk_size, 0);
    (chunk_src.0..chunk_dst.0).flat_map(move |i| (chunk_src.1..chunk_dst.1).map(move |j| (i, j)))
}

fn rect_chunks_stack_iter(
    rect: Rectangle,
    chunk_size: UVec2,
    mipmap_cap: u8,
) -> impl Iterator<Item = (i32, i32, u8)> {
    (0..mipmap_cap).flat_map(move |m| rect_chunks_stack_layer_iter(rect, chunk_size, m))
}

fn rect_chunks_stack_layer_iter(
    rect: Rectangle,
    chunk_size: UVec2,
    mipmap: u8,
) -> impl Iterator<Item = (i32, i32, u8)> {
    let (chunk_src, chunk_dst) = rect_chunks_stack_layer_range(rect, chunk_size, mipmap);
    (chunk_src.0..chunk_dst.0)
        .flat_map(move |i| (chunk_src.1..chunk_dst.1).map(move |j| (i, j, mipmap)))
}

fn rect_chunks_stack_layer_range(
    rect: Rectangle,
    chunk_size: UVec2,
    mipmap: u8,
) -> ((i32, i32), (i32, i32)) {
    let size = (chunk_size << mipmap).as_ivec2();
    let chunk_src = rect.left_down().div_euclid(size);
    let chunk_dst = (rect.right_up() - 1).div_euclid(size) + 1;
    (chunk_src.into(), chunk_dst.into())
}

fn chunks_key_to_rect(key: (i32, i32), chunk_size: UVec2) -> Rectangle {
    Rectangle {
        origin: IVec2::new(key.0, key.1) * chunk_size.as_ivec2(),
        extend: chunk_size,
    }
}

fn chunks_stack_key_to_rect(key: (i32, i32, u8), chunk_size: UVec2) -> Rectangle {
    let size = chunk_size << key.2;
    Rectangle {
        origin: IVec2::new(key.0, key.1) * size.as_ivec2(),
        extend: size,
    }
}

fn upper_chunk_of(chunk: (i32, i32, u8)) -> (i32, i32, u8) {
    (chunk.0.div_euclid(2), chunk.1.div_euclid(2), chunk.2 + 1)
}

#[expect(unused)]
fn lower_chunk_of(chunk: (i32, i32, u8)) -> [(i32, i32, u8); 4] {
    [
        (chunk.0 * 2, chunk.1 * 2, chunk.2 - 1),
        (chunk.0 * 2 + 1, chunk.1 * 2, chunk.2 - 1),
        (chunk.0 * 2, chunk.1 * 2 + 1, chunk.2 - 1),
        (chunk.0 * 2 + 1, chunk.1 * 2 + 1, chunk.2 - 1),
    ]
}

// --- Layouts --- //

const LAYOUT_SAMPLER: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
    label: Some("layer_sampler"),
    entries: &[BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    }],
};

const LAYOUT_WORK_DISPATCH: BindGroupLayoutDescriptor<'_> = BindGroupLayoutDescriptor {
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

fn work_dispatch_group(device: &Device, dispatch_layout: &BindGroupLayout) -> (Buffer, BindGroup) {
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

fn create_chunk_texture(device: &Device, size: UVec2) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("layer_chunk_texture"),
        size: Extent3d {
            width: size.x,
            height: size.y,
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
        view_formats: &[TextureFormat::Rgba8UnormSrgb],
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
        format: Some(TextureFormat::Rgba8UnormSrgb),
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

    let read_write = chunk_layout.read_write.as_ref().map(|layout| {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("layer_chunk_read_write"),
            layout,
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
        })
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

struct RenderPipelines {
    over: RenderPipeline,
    debug: RenderPipeline,
}

struct MergePipelines {
    over: ComputePipeline,
    replace: ComputePipeline,
    erase: ComputePipeline,
    over_in_place: Option<ComputePipeline>,
    replace_in_place: Option<ComputePipeline>,
    erase_in_place: Option<ComputePipeline>,
}

struct BrushPipelines {
    blur: ComputePipeline,
    smudge: ComputePipeline,
    smudge_prepare_stroke: ComputePipeline,
    smudge_prepare_draw: ComputePipeline,
    tint: ComputePipeline,
    round_over: ComputePipeline,
    round_erase: ComputePipeline,
    pixel_replace: ComputePipeline,
    pixel_erase: ComputePipeline,
    tint_in_place: Option<ComputePipeline>,
    blur_in_place: Option<ComputePipeline>,
    smudge_in_place: Option<ComputePipeline>,
    round_over_in_place: Option<ComputePipeline>,
    round_erase_in_place: Option<ComputePipeline>,
    pixel_replace_in_place: Option<ComputePipeline>,
    pixel_erase_in_place: Option<ComputePipeline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BrushKind {
    Blur,
    Pixel,
    Smudge,
    Tint,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BrushStage {
    PrepareStroke,
    PrepareDraw,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WriteMode {
    InPlace,
    OutOfPlace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CompositeMode {
    Over,
    Erase,
    Replace,
}

impl MergePipelines {
    fn get(&self, write: WriteMode, composite: CompositeMode) -> &ComputePipeline {
        use CompositeMode::*;
        use WriteMode::*;

        match (write, composite) {
            (InPlace, Over) => self.over_in_place.as_ref().unwrap(),
            (InPlace, Erase) => self.erase_in_place.as_ref().unwrap(),
            (InPlace, Replace) => self.replace_in_place.as_ref().unwrap(),
            (OutOfPlace, Over) => &self.over,
            (OutOfPlace, Erase) => &self.erase,
            (OutOfPlace, Replace) => &self.replace,
        }
    }
}

impl BrushPipelines {
    fn get(
        &self,
        brush: BrushKind,
        stage: BrushStage,
        write: WriteMode,
        composite: CompositeMode,
    ) -> &ComputePipeline {
        use BrushKind::*;
        use BrushStage::*;
        use CompositeMode::*;
        use WriteMode::*;

        match (brush, stage, write, composite) {
            (Blur, Draw, OutOfPlace, Over) => &self.blur,
            (Smudge, Draw, OutOfPlace, Over) => &self.smudge,
            (Smudge, PrepareStroke, InPlace | OutOfPlace, Over) => &self.smudge_prepare_stroke,
            (Smudge, PrepareDraw, InPlace | OutOfPlace, Over) => &self.smudge_prepare_draw,
            (Tint, Draw, OutOfPlace, Over) => &self.tint,
            (Round, Draw, OutOfPlace, Over) => &self.round_over,
            (Round, Draw, OutOfPlace, Erase) => &self.round_erase,
            (Pixel, Draw, OutOfPlace, Replace) => &self.pixel_replace,
            (Pixel, Draw, OutOfPlace, Erase) => &self.pixel_erase,
            (Blur, Draw, InPlace, Over) => &self.blur_in_place.as_ref().unwrap(),
            (Smudge, Draw, InPlace, Over) => &self.smudge_in_place.as_ref().unwrap(),
            (Tint, Draw, InPlace, Over) => self.tint_in_place.as_ref().unwrap(),
            (Round, Draw, InPlace, Over) => self.round_over_in_place.as_ref().unwrap(),
            (Round, Draw, InPlace, Erase) => self.round_erase_in_place.as_ref().unwrap(),
            (Pixel, Draw, InPlace, Replace) => self.pixel_replace_in_place.as_ref().unwrap(),
            (Pixel, Draw, InPlace, Erase) => self.pixel_erase_in_place.as_ref().unwrap(),
            it => panic!("brush pipeline unavailable {it:?}"),
        }
    }
}

impl BrushKind {
    const fn label(self) -> &'static str {
        match self {
            BrushKind::Blur => "blur",
            BrushKind::Pixel => "pixel",
            BrushKind::Smudge => "smudge",
            BrushKind::Tint => "tint",
            BrushKind::Round => "round",
        }
    }

    fn shader(self, stage: BrushStage) -> &'static str {
        use BrushKind::*;
        use BrushStage::*;

        match (self, stage) {
            (Blur, Draw) => include_str!("layer/brush/blur.wgsl"),
            (Pixel, Draw) => include_str!("layer/brush/pixel.wgsl"),
            (Round, Draw) => include_str!("layer/brush/round.wgsl"),
            (Smudge, Draw) => include_str!("layer/brush/smudge.wgsl"),
            (Smudge, PrepareDraw) => include_str!("layer/brush/smudge_prepare_draw.wgsl"),
            (Smudge, PrepareStroke) => include_str!("layer/brush/smudge_prepare_stroke.wgsl"),
            (Tint, Draw) => include_str!("layer/brush/tint.wgsl"),
            it => panic!("brush pipeline unavailable {it:?}"),
        }
    }
}

impl BrushStage {
    const fn label(self) -> &'static str {
        match self {
            BrushStage::PrepareStroke => "prepare_stroke",
            BrushStage::PrepareDraw => "prepare_draw",
            BrushStage::Draw => "draw",
        }
    }
}

impl WriteMode {
    const fn read(self) -> &'static str {
        match self {
            WriteMode::InPlace => "read_write",
            WriteMode::OutOfPlace => "read",
        }
    }

    const fn write(self) -> &'static str {
        match self {
            WriteMode::InPlace => "read_write",
            WriteMode::OutOfPlace => "write",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            WriteMode::InPlace => "read_write",
            WriteMode::OutOfPlace => "swap",
        }
    }
}

impl CompositeMode {
    const fn formula(self) -> &'static str {
        match self {
            CompositeMode::Over => "src + dst * (1 - src.a)",
            CompositeMode::Erase => "dst * (1 - src.a)",
            CompositeMode::Replace => "src",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            CompositeMode::Over => "over",
            CompositeMode::Erase => "erase",
            CompositeMode::Replace => "replace",
        }
    }
}

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
        over: new_pipeline(BlendState::REPLACE, "layer_chunk_over", "fs_main"),
        debug: new_pipeline(BlendState::REPLACE, "layer_chunk_over_debug", "fs_debug"),
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

fn merge_pipelines(device: &Device, chunk_layout: &ChunkLayout) -> MergePipelines {
    let layout_out_of_place = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_merge"),
        bind_group_layouts: &[
            Some(&chunk_layout.dispatch),
            Some(&chunk_layout.read),
            Some(&chunk_layout.read),
            Some(&chunk_layout.write),
        ],
        immediate_size: 0,
    });

    let layout_in_place = chunk_layout.read_write.as_ref().map(|read_write| {
        device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("layer_merge"),
            bind_group_layouts: &[
                Some(&chunk_layout.dispatch),
                Some(read_write),
                Some(&chunk_layout.read),
                Some(read_write),
            ],
            immediate_size: 0,
        })
    });

    MergePipelines {
        over: merge_pipeline(
            device,
            &layout_out_of_place,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        replace: merge_pipeline(
            device,
            &layout_out_of_place,
            WriteMode::OutOfPlace,
            CompositeMode::Replace,
        ),
        erase: merge_pipeline(
            device,
            &layout_out_of_place,
            WriteMode::OutOfPlace,
            CompositeMode::Erase,
        ),
        over_in_place: layout_in_place
            .as_ref()
            .map(|layout| merge_pipeline(device, layout, WriteMode::InPlace, CompositeMode::Over)),
        replace_in_place: layout_in_place.as_ref().map(|layout| {
            merge_pipeline(device, layout, WriteMode::InPlace, CompositeMode::Replace)
        }),
        erase_in_place: layout_in_place
            .as_ref()
            .map(|layout| merge_pipeline(device, layout, WriteMode::InPlace, CompositeMode::Erase)),
    }
}

fn merge_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    write: WriteMode,
    composite: CompositeMode,
) -> ComputePipeline {
    let label = format!("layer_merge_{}_{}", write.label(), composite.label());
    let maps = &[
        ("read", write.read()),
        ("write", write.write()),
        ("composite", composite.formula()),
    ];
    let wgsl = shader_compile(include_str!("layer/merge.wgsl"), maps);
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(&label),
        source: ShaderSource::Wgsl(wgsl.into()),
    });
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some(&label),
        layout: Some(layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
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
    dispatch_draw_layout: &BindGroupLayout,
    chunk_layout: &ChunkLayout,
) -> BrushPipelines {
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

    let layout_out_of_place = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("layer_brush"),
        bind_group_layouts: &[
            Some(dispatch_draw_layout),
            Some(&chunk_layout.read),
            Some(&chunk_layout.write),
        ],
        immediate_size: 0,
    });

    let layout_in_place = chunk_layout.read_write.as_ref().map(|read_write| {
        device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("layer_brush"),
            bind_group_layouts: &[
                Some(dispatch_draw_layout),
                Some(read_write),
                Some(read_write),
            ],
            immediate_size: 0,
        })
    });

    BrushPipelines {
        blur: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Blur,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        smudge: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Smudge,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        smudge_prepare_stroke: brush_pipeline(
            device,
            &layout_prepare_stroke,
            BrushKind::Smudge,
            BrushStage::PrepareStroke,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        smudge_prepare_draw: brush_pipeline(
            device,
            &layout_prepare_draw,
            BrushKind::Smudge,
            BrushStage::PrepareDraw,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        tint: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Tint,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Erase,
        ),
        round_over: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Round,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Over,
        ),
        round_erase: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Round,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Erase,
        ),
        pixel_replace: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Pixel,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Replace,
        ),
        pixel_erase: brush_pipeline(
            device,
            &layout_out_of_place,
            BrushKind::Pixel,
            BrushStage::Draw,
            WriteMode::OutOfPlace,
            CompositeMode::Erase,
        ),
        tint_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Tint,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Over,
            )
        }),
        blur_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Blur,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Over,
            )
        }),
        smudge_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Smudge,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Over,
            )
        }),
        round_over_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Round,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Over,
            )
        }),
        round_erase_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Round,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Erase,
            )
        }),
        pixel_replace_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Pixel,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Replace,
            )
        }),
        pixel_erase_in_place: layout_in_place.as_ref().map(|layout| {
            brush_pipeline(
                device,
                layout,
                BrushKind::Pixel,
                BrushStage::Draw,
                WriteMode::InPlace,
                CompositeMode::Erase,
            )
        }),
    }
}

fn brush_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    brush: BrushKind,
    stage: BrushStage,
    write: WriteMode,
    composite: CompositeMode,
) -> ComputePipeline {
    let label = format!(
        "layer_brush_{}_{}_{}_{}",
        brush.label(),
        stage.label(),
        write.label(),
        composite.label()
    );
    let maps = &[
        ("read", write.read()),
        ("write", write.write()),
        ("composite", composite.formula()),
    ];
    let wgsl = shader_compile(brush.shader(stage), maps);
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(&label),
        source: ShaderSource::Wgsl(wgsl.into()),
    });
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some(&label),
        layout: Some(layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    })
}
