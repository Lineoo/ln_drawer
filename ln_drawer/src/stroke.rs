pub mod chunk;
pub mod dirty;
pub mod interpolate;
pub mod modifier;
pub mod shape;

use std::{
    error::Error,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError, channel},
    thread::JoinHandle,
};

use hashbrown::{HashMap, HashSet};
use indexmap::IndexSet;
use ln_world::{Element, Handle, World};
use palette::Srgba;
use redb::{Database, ReadableDatabase, TableDefinition};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, FilterMode, FragmentState, MapMode, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PollType, PrimitiveState,
    PrimitiveTopology, Queue, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase,
    Texture, TextureAspect, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::{TextureDescriptor, TextureViewDescriptor},
};
use winit::event::PointerKind;

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position, PositionFract, Rectangle, Size},
    render::{
        MSAA_STATE, Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged, CameraUtils},
        vertex::VertexUniform,
    },
    save::{Autosave, SaveDatabase},
    stroke::{
        dirty::Dirty,
        interpolate::{Draw, Interpolation},
        modifier::{DrawProcessedStorage, Modifier},
        shape::{PixelBrush, RoundBrush},
    },
    tools::{
        collider::ToolCollider,
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
};

const CHUNK_SIZE: u32 = 512;
const CHUNK_CAPS: usize = 2048;
const CHUNK_BATCH: usize = 8;
const CHUNK_MIPMAP: u8 = 8;
const MAX_STROKE: u64 = 200;

const TABLE_STROKE_CHUNK: TableDefinition<ChunkKey, &[u8]> = TableDefinition::new("stroke_chunk");

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
            draw.position.round(),
            Size::splat((draw.size * 2.0).ceil() as u32),
        )
    },
};

type ChunkKey = (i32, i32, u8);

pub struct StrokeLayer {
    chunks: HashMap<ChunkKey, Option<Chunk>>,
    mipmap_ready: HashSet<ChunkKey>,

    render_pipeline: RenderPipeline,
    render_sampler: Sampler,
    render_layout: BindGroupLayout,

    mipmap_pipeline: ComputePipeline,
    mipmap_layout: BindGroupLayout,
    mipmap_meta: BindGroup,
    mipmap_meta_buffer: Buffer,

    compute_layout: BindGroupLayout,
    dispatch: BindGroup,
    dispatch_meta: Buffer,
    draws_array: Buffer,

    thread_tx: Sender<ThreadInput>,
    thread_rx: Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,

    pub interpolation: Interpolation,
    pub modifier: Modifier,
    pub dirty: Dirty,
    pub shape: u32,
    pub brush_round: RoundBrush,
    pub brush_pixel: PixelBrush,
    prev: Option<Draw>,
}

#[derive(Clone)]
struct Chunk {
    texture: Texture,
    uniform: Buffer,
    render: BindGroup,
    mipmap: Option<BindGroup>,
    compute: BindGroup,
}

enum ThreadInput {
    SetStreamCenter(ChunkKey),
    SetStreamSize(Size),
    MarkUnsaved(ChunkKey),
    Create(ChunkKey, Texture),
    Autosave,
    Finish,
}

enum ThreadOutput {
    Insert(ChunkKey, Option<Texture>),
    Remove(ChunkKey),
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    dirty_coords: [i32; 2],
    stroke_count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MipmapUniform {
    mipmap_coords: [i32; 2],
    mipmap_size: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    chunk: [i32; 3],
    _pad: u32,
}

impl StrokeLayer {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let render_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("stroke_chunk_fragment"),
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
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let compute_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("stroke_chunk_compute"),
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

        let mipmap_meta_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("mipmap_meta"),
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

        let mipmap_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("mipmap"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
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
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
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
            label: Some("dispatch"),
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
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let brush_round = RoundBrush::new(&render, &dispatch_layout, &compute_layout);
        let brush_pixel = PixelBrush::new(&render, &dispatch_layout, &compute_layout);

        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("stroke_chunk"),
            source: ShaderSource::Wgsl(include_str!("stroke/chunk.wgsl").into()),
        });

        let render_pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("stroke_chunk"),
            bind_group_layouts: &[&camera_bind.layout, &render_layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("stroke_chunk"),
            layout: Some(&render_pipeline),
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
                    format: render.config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        });

        let render_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("stroke_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let mipmap_meta_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("mipmap_meta"),
            size: size_of::<MipmapUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mipmap_meta = device.create_bind_group(&BindGroupDescriptor {
            label: Some("mipmap_meta"),
            layout: &mipmap_meta_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &mipmap_meta_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let mipmap_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("stroke_chunk"),
            source: ShaderSource::Wgsl(include_str!("stroke/mipmap.wgsl").into()),
        });

        let mipmap_pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("stroke_mipmap"),
            bind_group_layouts: &[&mipmap_meta_layout, &mipmap_layout],
            immediate_size: 0,
        });

        let mipmap_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("stroke_mipmap"),
            layout: Some(&mipmap_pipeline),
            module: &mipmap_shader,
            entry_point: Some("cs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let dispatch_meta = device.create_buffer(&BufferDescriptor {
            label: Some("dispatch_meta"),
            size: size_of::<DispatchUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_array = device.create_buffer(&BufferDescriptor {
            label: Some("draws_array"),
            size: size_of::<DrawProcessedStorage>() as u64 * MAX_STROKE,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dispatch = device.create_bind_group(&BindGroupDescriptor {
            label: Some("dispatch"),
            layout: &dispatch_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &dispatch_meta,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &draws_array,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let (thread_input_tx, thread_input_rx) = channel();
        let (thread_output_tx, thread_output_rx) = channel();

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();
        let camera = world.single_fetch::<Camera>().unwrap();
        let render = world.single_fetch::<Render>().unwrap();
        let device = render.device.clone();
        let queue = render.queue.clone();

        let chunk_here = chunk_of(camera.center.round());

        thread_input_tx
            .send(ThreadInput::SetStreamCenter(chunk_here))
            .unwrap();
        thread_input_tx
            .send(ThreadInput::SetStreamSize(camera.size))
            .unwrap();

        let thread = std::thread::spawn(|| {
            Self::loading_thread(database, device, queue, thread_input_rx, thread_output_tx)
                .unwrap();
        });

        StrokeLayer {
            chunks: HashMap::new(),
            mipmap_ready: HashSet::new(),
            render_sampler,
            render_pipeline,
            render_layout,
            mipmap_pipeline,
            mipmap_layout,
            mipmap_meta,
            mipmap_meta_buffer,
            compute_layout,
            dispatch,
            dispatch_meta,
            draws_array,
            thread_tx: thread_input_tx,
            thread_rx: thread_output_rx,
            thread: Some(thread),
            interpolation: DEFAULT_INTERPOLATION,
            modifier: DEFAULT_MODIFIER,
            dirty: DEFAULT_DIRTY,
            shape: 0,
            brush_round,
            brush_pixel,
            prev: None,
        }
    }

    fn create_chunk(&self, device: &Device, key: ChunkKey) -> Chunk {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("stroke_chunk_texture"),
            size: Extent3d {
                width: CHUNK_SIZE,
                height: CHUNK_SIZE,
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
            view_formats: &[],
        });

        Self::create_chunk_from_texture(
            &self.render_layout,
            &self.render_sampler,
            &self.compute_layout,
            texture,
            device,
            key,
        )
    }

    fn create_chunk_from_texture(
        render_layout: &BindGroupLayout,
        render_sampler: &Sampler,
        compute_layout: &BindGroupLayout,
        texture: Texture,
        device: &Device,
        key: ChunkKey,
    ) -> Chunk {
        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("stroke_chunk_rectangle"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: [
                    key.0 * CHUNK_SIZE as i32 * 2i32.pow(key.2 as u32),
                    key.1 * CHUNK_SIZE as i32 * 2i32.pow(key.2 as u32),
                ],
                extend: [
                    CHUNK_SIZE * 2u32.pow(key.2 as u32),
                    CHUNK_SIZE * 2u32.pow(key.2 as u32),
                ],
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("stroke_chunk_key"),
            contents: bytemuck::bytes_of(&ChunkUniform {
                chunk: [key.0, key.1, key.2 as i32],
                _pad: 0,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let texture_view = texture.create_view(&TextureViewDescriptor {
            label: Some("stroke_chunk_texture_view"),
            ..Default::default()
        });

        let render_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("stroke_chunk_render"),
            layout: &render_layout,
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
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&render_sampler),
                },
            ],
        });

        let compute_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("stroke_chunk_compute"),
            layout: &compute_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
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
            texture,
            uniform,
            render: render_bind,
            mipmap: None,
            compute: compute_bind,
        }
    }

    fn chunk_bind_mipmap(
        mipmap_layout: &BindGroupLayout,
        lower: &mut Chunk,
        upper: &Chunk,
        device: &Device,
    ) {
        debug_assert!(lower.mipmap.is_none());

        let mipmap_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("mipmap"),
            layout: &mipmap_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&upper.texture.create_view(
                        &TextureViewDescriptor {
                            label: Some("destination"),
                            ..Default::default()
                        },
                    )),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &upper.uniform,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&lower.texture.create_view(
                        &TextureViewDescriptor {
                            label: Some("source"),
                            ..Default::default()
                        },
                    )),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &lower.uniform,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        lower.mipmap = Some(mipmap_bind);
    }

    fn chunk_readback(texture: &Texture, device: &Device, queue: &Queue) -> Vec<u8> {
        let (tx, rx) = std::sync::mpsc::channel();

        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("chunk_readback"),
            size: (CHUNK_SIZE * CHUNK_SIZE * 4) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("chunk_readback"),
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfoBase {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfoBase {
                buffer: &readback_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(CHUNK_SIZE * 4),
                    rows_per_image: Some(CHUNK_SIZE),
                },
            },
            Extent3d {
                width: CHUNK_SIZE,
                height: CHUNK_SIZE,
                depth_or_array_layers: 1,
            },
        );

        let command = encoder.finish();

        queue.submit([command]);

        let inner = readback_buffer.clone();
        readback_buffer.map_async(MapMode::Read, .., move |ret| {
            ret.unwrap();

            let view = inner.get_mapped_range(..);
            tx.send(view.to_vec()).unwrap();
        });

        device.poll(PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap()
    }

    fn database_init(&mut self, db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        write.open_table(TABLE_STROKE_CHUNK)?;
        write.commit()?;
        Ok(())
    }

    fn attach_autosave(&mut self, world: &World, this: Handle<Self>) {
        let save = world.insert(Autosave(Box::new(move |world, _| {
            let this = world.fetch_mut(this).unwrap();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let camera = world.single::<Camera>().unwrap();
        world.observer(camera, move |change: &CameraPositionChanged, world| {
            let chunk_from = chunk_of(change.from.round());
            let chunk_here = chunk_of(change.here.round());

            if chunk_from != chunk_here {
                let this = world.fetch(this).unwrap();
                let camera = world.fetch(camera).unwrap();
                this.thread_tx
                    .send(ThreadInput::SetStreamCenter(chunk_here))
                    .unwrap();
                this.thread_tx
                    .send(ThreadInput::SetStreamSize(camera.size))
                    .unwrap();
            }
        });
    }

    fn attach_render(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this).unwrap();
                while let Ok(output) = this.thread_rx.try_recv() {
                    this.process_thread_output(world, output);
                }

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(|world, rpass| {
                let stroke = world.single_fetch::<StrokeLayer>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let view_rect = camera.world_view_rect();
                let mipmap = ((-camera.zoom.floor()).max(0) as u8).min(CHUNK_MIPMAP - 1);
                let (chunk_src, chunk_dst) = view_rect_to_chunk(view_rect, mipmap);

                rpass.set_pipeline(&stroke.render_pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);

                let mut chunk_list = Vec::new();
                for chunk_x in chunk_src.0..chunk_dst.0 {
                    for chunk_y in chunk_src.1..chunk_dst.1 {
                        chunk_list.push((chunk_x, chunk_y, mipmap));
                    }
                }

                while let Some(chunk) = chunk_list.pop() {
                    if let Some(possible) = stroke.chunks.get(&chunk) {
                        if let Some(chunk) = possible {
                            rpass.set_bind_group(1, &chunk.render, &[]);
                            rpass.draw(0..4, 0..1);
                        } else if chunk.2 > 0 && chunk.2 + 3 > mipmap {
                            let (rx, ry, rp) = lower_root_chunk_of(chunk);

                            chunk_list.push((rx, ry, rp));
                            chunk_list.push((rx + 1, ry, rp));
                            chunk_list.push((rx, ry + 1, rp));
                            chunk_list.push((rx + 1, ry + 1, rp));
                        }
                    }
                }
            })),
        });
        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }

    fn process_thread_output(&mut self, world: &World, output: ThreadOutput) {
        match output {
            ThreadOutput::Insert(chunk_id, texture) => {
                let render = world.single_fetch::<Render>().unwrap();
                let chunk = texture.map(|texture| {
                    let mut new_chunk = Self::create_chunk_from_texture(
                        &self.render_layout,
                        &self.render_sampler,
                        &self.compute_layout,
                        texture,
                        &render.device,
                        chunk_id,
                    );

                    self.update_mipmap_bind(chunk_id, &mut new_chunk, &render);

                    new_chunk
                });

                self.chunks.insert(chunk_id, chunk);
                self.detect_corrupted(chunk_id, &render);
            }
            ThreadOutput::Remove(chunk_id) => {
                self.chunks.remove(&chunk_id);
                self.mipmap_ready.remove(&chunk_id);

                if chunk_id.2 > 0 {
                    let (rx, ry, rp) = lower_root_chunk_of(chunk_id);

                    if let Some(Some(c)) = self.chunks.get_mut(&(rx, ry, rp)) {
                        c.mipmap = None;
                    }
                    if let Some(Some(c)) = self.chunks.get_mut(&(rx + 1, ry, rp)) {
                        c.mipmap = None;
                    }
                    if let Some(Some(c)) = self.chunks.get_mut(&(rx, ry + 1, rp)) {
                        c.mipmap = None;
                    }
                    if let Some(Some(c)) = self.chunks.get_mut(&(rx + 1, ry + 1, rp)) {
                        c.mipmap = None;
                    }
                }
            }
        }
    }

    fn detect_corrupted(&mut self, upper: (i32, i32, u8), render: &Render) {
        // If the lower layer has content but upper layer is empty, then something must go wrong.
        // We will regen mipmap in that case.

        if let Some(None) = self.chunks.get(&upper)
            && upper.2 > 0
        {
            let (rx, ry, rp) = lower_root_chunk_of(upper);

            let r0 = (rx, ry, rp);
            let r1 = (rx + 1, ry, rp);
            let r2 = (rx, ry + 1, rp);
            let r3 = (rx + 1, ry + 1, rp);

            let p0 = self.chunks.get(&r0);
            let p1 = self.chunks.get(&r1);
            let p2 = self.chunks.get(&r2);
            let p3 = self.chunks.get(&r3);

            let all_loaded = p0.is_some() && p1.is_some() && p2.is_some() && p3.is_some();
            let all_ready = self.mipmap_ready.contains(&r0)
                && self.mipmap_ready.contains(&r1)
                && self.mipmap_ready.contains(&r2)
                && self.mipmap_ready.contains(&r3);

            if all_loaded && all_ready {
                let any_content = p0.is_some_and(|x| x.is_some())
                    || p1.is_some_and(|x| x.is_some())
                    || p2.is_some_and(|x| x.is_some())
                    || p3.is_some_and(|x| x.is_some());

                if any_content {
                    self.fix_corrupted_mipmap(render, upper);
                }

                self.mipmap_ready.insert(upper);
                self.detect_corrupted(upper_chunk_of(upper), render);
            }
        } else if self
            .chunks
            .get(&upper)
            .is_some_and(|x| x.is_some() || upper.2 == 0)
        {
            self.mipmap_ready.insert(upper);
            self.detect_corrupted(upper_chunk_of(upper), &render);
        }
    }

    fn update_mipmap_bind(&mut self, chunk_id: (i32, i32, u8), chunk: &mut Chunk, render: &Render) {
        if chunk_id.2 > 0 {
            let (rx, ry, rp) = lower_root_chunk_of(chunk_id);

            if let Some(Some(lower)) = self.chunks.get_mut(&(rx, ry, rp)) {
                Self::chunk_bind_mipmap(&self.mipmap_layout, lower, chunk, &render.device);
            }

            if let Some(Some(lower)) = self.chunks.get_mut(&(rx + 1, ry, rp)) {
                Self::chunk_bind_mipmap(&self.mipmap_layout, lower, chunk, &render.device);
            }

            if let Some(Some(lower)) = self.chunks.get_mut(&(rx, ry + 1, rp)) {
                Self::chunk_bind_mipmap(&self.mipmap_layout, lower, chunk, &render.device);
            }

            if let Some(Some(lower)) = self.chunks.get_mut(&(rx + 1, ry + 1, rp)) {
                Self::chunk_bind_mipmap(&self.mipmap_layout, lower, chunk, &render.device);
            }
        }

        if let Some(Some(upper)) = self.chunks.get(&upper_chunk_of(chunk_id)) {
            Self::chunk_bind_mipmap(&self.mipmap_layout, chunk, upper, &render.device);
        }
    }

    fn attach_touch(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider::fullscreen(-100));
        world.dependency(collider, this);

        let mut pinch_distance = None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let primary = event.members.first().unwrap();

            if matches!(event.active.pointer, PointerKind::Touch(_)) || event.members.len() != 1 {
                let mut sum = [0f64; 2];
                for member in &event.members {
                    sum[0] += member.screen[0];
                    sum[1] += member.screen[1];
                }

                let cnt = event.members.len() as f64;
                let center = [sum[0] / cnt, sum[1] / cnt];

                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                match event.active.status {
                    MultiTouchStatus::Press => {
                        camera_utils.locked(false);
                        camera_utils.cursor(world, center);
                        camera_utils.anchor_on_screen(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Holding => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Release => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(false);
                    }
                }

                if event.members.len() == 2 {
                    let first = event.members.first().unwrap().screen;
                    let last = event.members.last().unwrap().screen;

                    let (x, y) = (first[0] - last[0], first[1] - last[1]);
                    let cur = (x * x + y * y).sqrt();
                    let prev = pinch_distance.get_or_insert(cur);
                    camera_utils.zoom_delta(world, Fract::from_f64((cur - *prev) * 2.0));
                    *prev = cur;
                } else {
                    pinch_distance = None;
                }
            } else if let MultiTouchStatus::Holding | MultiTouchStatus::Press = primary.status {
                let mut this = world.fetch_mut(this).unwrap();
                let target = Draw {
                    position: primary.position,
                    force: primary.data.force.unwrap_or(1.0),
                };

                this.paint(target, world);
            } else {
                world.queue(move |world| {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.prev = None;
                });
            }
        });
    }

    fn paint(&mut self, next: Draw, world: &World) {
        // generate draws //

        let mut draw_buf = Vec::new();
        let curr = self
            .interpolation
            .interpolate(self.prev, next, &self.modifier, &mut draw_buf);
        self.prev = Some(curr);

        let dirty = self.dirty.compute(curr.position.round(), &draw_buf);
        if dirty.extend.w == 0 || dirty.extend.h == 0 {
            return;
        }

        // pre-check that chunks are all ready

        for mipmap in 0..CHUNK_MIPMAP {
            let (chunk_src, chunk_dst) = view_rect_to_chunk(dirty, mipmap);
            for chunk_x in chunk_src.0..chunk_dst.0 {
                for chunk_y in chunk_src.1..chunk_dst.1 {
                    let chunk_id = (chunk_x, chunk_y, mipmap);

                    if let None = self.chunks.get(&chunk_id) {
                        return;
                    }
                }
            }
        }

        // prepare chunks

        let mut paint_chunks = Vec::new();
        let mut mipmap_chunks = Vec::new();

        for mipmap in 0..CHUNK_MIPMAP {
            let (chunk_src, chunk_dst) = view_rect_to_chunk(dirty, mipmap);
            for chunk_x in chunk_src.0..chunk_dst.0 {
                for chunk_y in chunk_src.1..chunk_dst.1 {
                    let chunk_id = (chunk_x, chunk_y, mipmap);

                    if let Some(chunk) = self.chunks.get(&chunk_id) {
                        if chunk.is_none() {
                            let render = world.single_fetch::<Render>().unwrap();
                            let mut chunk = self.create_chunk(&render.device, chunk_id);

                            self.update_mipmap_bind(chunk_id, &mut chunk, &render);

                            self.thread_tx
                                .send(ThreadInput::Create(chunk_id, chunk.texture.clone()))
                                .unwrap();
                            self.chunks.insert(chunk_id, Some(chunk));
                        }

                        self.thread_tx
                            .send(ThreadInput::MarkUnsaved(chunk_id))
                            .unwrap();

                        mipmap_chunks.push(chunk_id);
                        if mipmap == 0 {
                            paint_chunks.push(chunk_id);
                        }
                    }
                }
            }
        }

        // assign works to GPU

        let dispatch = DispatchUniform {
            dirty_coords: dirty.origin.into_array(),
            stroke_count: draw_buf.len() as u32,
            _pad: 0,
        };

        let mipmap = MipmapUniform {
            mipmap_coords: dirty.origin.into_array(),
            mipmap_size: dirty.extend.into_array(),
        };

        let render = world.single_fetch::<Render>().unwrap();
        let queue = &render.queue;
        let device = &render.device;

        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        queue.write_buffer(&self.dispatch_meta, 0, bytemuck::bytes_of(&dispatch));
        queue.write_buffer(&self.mipmap_meta_buffer, 0, bytemuck::bytes_of(&mipmap));
        queue.write_buffer(&self.draws_array, 0, bytemuck::cast_slice(&draw_stg));

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("stroke"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("stroke"),
            timestamp_writes: None,
        });

        const WORKGROUP_SIZE: Size = Size::new(16, 16);

        match self.shape {
            0 => cpass.set_pipeline(&self.brush_round.pipeline),
            1 => cpass.set_pipeline(&self.brush_pixel.pipeline),
            _ => unreachable!(),
        }
        cpass.set_bind_group(0, Some(&self.dispatch), &[]);
        for chunk in paint_chunks {
            let chunk = self.chunks.get(&chunk).unwrap().as_ref().unwrap();

            cpass.set_bind_group(1, Some(&chunk.compute), &[]);
            cpass.dispatch_workgroups(
                (dirty.extend.w - 1) / WORKGROUP_SIZE.w + 1,
                (dirty.extend.h - 1) / WORKGROUP_SIZE.h + 1,
                1,
            );
        }

        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.mipmap_meta), &[]);
        for chunk_id in mipmap_chunks {
            let chunk = self.chunks.get(&chunk_id).unwrap().as_ref().unwrap();

            if let Some(chunk_mipmap) = &chunk.mipmap {
                cpass.set_bind_group(1, Some(chunk_mipmap), &[]);
                cpass.dispatch_workgroups(
                    (dirty.extend.w - 1) / 2u32.pow(chunk_id.2 as u32) / WORKGROUP_SIZE.w + 1,
                    (dirty.extend.h - 1) / 2u32.pow(chunk_id.2 as u32) / WORKGROUP_SIZE.h + 1,
                    1,
                );
            }
        }

        drop(cpass);

        let command = encoder.finish();
        queue.submit([command]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }

    fn fix_corrupted_mipmap(&mut self, render: &Render, upper: (i32, i32, u8)) {
        log::warn!("fixing corrupted mipmap chunk {upper:?}");

        debug_assert!(self.chunks.get(&upper).is_some_and(|x| x.is_none()));

        let device = &render.device;
        let mut chunk = self.create_chunk(device, upper);

        self.update_mipmap_bind(upper, &mut chunk, &render);

        self.thread_tx
            .send(ThreadInput::Create(upper, chunk.texture.clone()))
            .unwrap();
        self.thread_tx
            .send(ThreadInput::MarkUnsaved(upper))
            .unwrap();
        self.chunks.insert(upper, Some(chunk));

        let (rx, ry, rp) = lower_root_chunk_of(upper);
        self.fix_corrupted(render, (rx, ry, rp));
        self.fix_corrupted(render, (rx, ry + 1, rp));
        self.fix_corrupted(render, (rx + 1, ry, rp));
        self.fix_corrupted(render, (rx + 1, ry + 1, rp));
    }

    fn fix_corrupted(&mut self, render: &Render, lower: (i32, i32, u8)) {
        const WORKGROUP_SIZE: Size = Size::new(16, 16);

        let Some(chunk) = self.chunks.get(&lower).unwrap() else {
            return;
        };

        let chunk_mipmap = chunk.mipmap.as_ref().unwrap();

        let (device, queue) = (&render.device, &render.queue);
        let size = chunk_size(lower.2);

        let mipmap = MipmapUniform {
            mipmap_coords: [lower.0 * size, lower.1 * size],
            mipmap_size: [size as u32; 2],
        };

        queue.write_buffer(&self.mipmap_meta_buffer, 0, bytemuck::bytes_of(&mipmap));

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("corrupted_fix"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("corrupted_fix"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.mipmap_meta), &[]);
        cpass.set_bind_group(1, Some(chunk_mipmap), &[]);
        cpass.dispatch_workgroups(
            (CHUNK_SIZE - 1) / WORKGROUP_SIZE.w + 1,
            (CHUNK_SIZE - 1) / WORKGROUP_SIZE.h + 1,
            1,
        );

        drop(cpass);

        let command = encoder.finish();
        queue.submit([command]);
    }

    fn loading_thread(
        database: SaveDatabase,
        device: Device,
        queue: Queue,
        input_rx: Receiver<ThreadInput>,
        output_tx: Sender<ThreadOutput>,
    ) -> Result<(), Box<dyn Error>> {
        let mut actual = HashMap::<ChunkKey, Option<Texture>>::new();
        let mut unsaved = HashSet::new();

        let mut tasks_buf = IndexSet::with_capacity(400);
        let mut task_frnt = 0;
        let mut task_batch;

        let mut ring = IndexSet::<ChunkKey>::new();
        let mut frnt = 0;

        let mut filt_load = IndexSet::new();
        let mut filt_unload = IndexSet::new();

        let (mut range_src, mut range_dst) = ((0, 0), (1, 1));

        loop {
            let input = if task_frnt < tasks_buf.len() {
                match input_rx.try_recv() {
                    Ok(input) => Some(input),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected)?,
                }
            } else {
                match input_rx.recv() {
                    Ok(input) => Some(input),
                    Err(RecvError) => Err(RecvError)?,
                }
            };

            match input {
                Some(ThreadInput::SetStreamCenter((chunk_center_x, chunk_center_y, mipmap))) => {
                    tasks_buf.clear();

                    for mipmap in 0..CHUNK_MIPMAP {
                        let mipmapped = (
                            chunk_center_x.div_euclid(2i32.pow(mipmap as u32)),
                            chunk_center_y.div_euclid(2i32.pow(mipmap as u32)),
                            mipmap,
                        );
                        for chunk_x in mipmapped.0 + range_src.0..mipmapped.0 + range_dst.0 {
                            for chunk_y in mipmapped.1 + range_src.1..mipmapped.1 + range_dst.1 {
                                tasks_buf.insert((chunk_x, chunk_y, mipmap));
                            }
                        }
                    }

                    tasks_buf.sort_by_key(|&(x, y, z)| {
                        let dx = (x - chunk_center_x).unsigned_abs();
                        let dy = (y - chunk_center_y).unsigned_abs();
                        let dz = (z as i32 - mipmap as i32).unsigned_abs();
                        dx.max(dy).max(dz)
                    });

                    task_frnt = 0;
                }
                Some(ThreadInput::SetStreamSize(size)) => {
                    let rect_min = Camera::manual_view_rect(Fract::ZERO, size, PositionFract::ZERO);
                    (range_src, _) = view_rect_to_chunk(rect_min, 0);

                    let pos = PositionFract::splat(Fract::new(CHUNK_SIZE as i32, 0));
                    let rect_max = Camera::manual_view_rect(Fract::ZERO, size, pos);
                    (_, range_dst) = view_rect_to_chunk(rect_max, 0);
                }
                Some(ThreadInput::MarkUnsaved(chunk)) => {
                    unsaved.insert(chunk);
                    continue;
                }
                Some(ThreadInput::Create(chunk_id, texture)) => {
                    debug_assert_eq!(actual.get(&chunk_id), Some(&None));
                    actual.insert(chunk_id, Some(texture));
                    continue;
                }
                Some(ThreadInput::Autosave) => {
                    let write = database.0.begin_write()?;
                    {
                        let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
                        for chunk_id in unsaved.drain() {
                            let Some(Some(texture)) = actual.get(&chunk_id) else {
                                continue;
                            };

                            let bytes = Self::chunk_readback(&texture, &device, &queue);
                            let compressed = zstd::encode_all(&bytes[..], 0)?;
                            table_chunk.insert(chunk_id, &compressed[..])?;
                        }
                    }
                    write.commit()?;
                    continue;
                }
                Some(ThreadInput::Finish) => {
                    return Ok(());
                }
                None => {}
            };

            task_batch = 0;
            while let Some(&key) = tasks_buf.get_index(task_frnt)
                && task_batch < CHUNK_BATCH
            {
                task_frnt += 1;

                while frnt < ring.len() && tasks_buf.contains(&ring[frnt]) {
                    // Keep required one in-place
                    frnt = (frnt + 1) % CHUNK_CAPS;
                }

                if frnt >= ring.len() {
                    // Out of bounds, just insert
                    if ring.insert(key) {
                        task_batch += 1;
                        filt_load.insert(key);
                    }

                    frnt = ring.len() % CHUNK_CAPS;
                    continue;
                }

                let Ok(replaced) = ring.replace_index(frnt, key) else {
                    // already loaded skipped
                    continue;
                };

                task_batch += 1;
                filt_load.swap_remove(&replaced);
                filt_unload.swap_remove(&key);
                filt_unload.insert(replaced);
                filt_load.insert(key);

                // move forward
                frnt = (frnt + 1) % CHUNK_CAPS;
            }

            let write = database.0.begin_write()?;
            for chunk_id in filt_unload.drain(..) {
                let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;

                if let Some(Some(chunk)) = actual.remove(&chunk_id) {
                    output_tx.send(ThreadOutput::Remove(chunk_id))?;

                    if unsaved.remove(&chunk_id) {
                        let bytes = Self::chunk_readback(&chunk, &device, &queue);
                        let compressed = zstd::encode_all(&bytes[..], 0)?;
                        table_chunk.insert(chunk_id, &compressed[..])?;
                    }
                }
            }
            write.commit()?;

            let read = database.0.begin_read()?;
            for chunk_id in filt_load.drain(..) {
                let table_chunk = read.open_table(TABLE_STROKE_CHUNK)?;

                if let Some(chunk) = table_chunk.get(chunk_id)? {
                    let bytes = zstd::decode_all(chunk.value())?;

                    let texture = device.create_texture(&TextureDescriptor {
                        label: Some("stroke_chunk_texture"),
                        size: Extent3d {
                            width: CHUNK_SIZE,
                            height: CHUNK_SIZE,
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
                        view_formats: &[],
                    });

                    queue.write_texture(
                        TexelCopyTextureInfoBase {
                            texture: &texture,
                            mip_level: 0,
                            origin: Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &bytes,
                        TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(CHUNK_SIZE * 4),
                            rows_per_image: Some(CHUNK_SIZE),
                        },
                        Extent3d {
                            width: CHUNK_SIZE,
                            height: CHUNK_SIZE,
                            depth_or_array_layers: 1,
                        },
                    );

                    actual.insert(chunk_id, Some(texture.clone()));
                    output_tx.send(ThreadOutput::Insert(chunk_id, Some(texture)))?;
                } else {
                    actual.insert(chunk_id, None);
                    output_tx.send(ThreadOutput::Insert(chunk_id, None))?;
                }
            }
        }
    }
}

fn chunk_size(mipmap: u8) -> i32 {
    CHUNK_SIZE as i32 * 2i32.pow(mipmap as u32)
}

fn view_rect_to_chunk(view_rect: Rectangle, mipmap: u8) -> ((i32, i32), (i32, i32)) {
    let size = chunk_size(mipmap);
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

fn lower_root_chunk_of(chunk: ChunkKey) -> ChunkKey {
    (chunk.0 * 2, chunk.1 * 2, chunk.2 - 1)
}

fn upper_chunk_of(chunk: ChunkKey) -> ChunkKey {
    (chunk.0.div_euclid(2), chunk.1.div_euclid(2), chunk.2 + 1)
}

fn chunk_of(center: Position) -> ChunkKey {
    (
        center.x.div_euclid(CHUNK_SIZE as i32),
        center.y.div_euclid(CHUNK_SIZE as i32),
        0,
    )
}

impl Element for StrokeLayer {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        // ensure singleton
        world.single::<StrokeLayer>().unwrap();

        let db = world.single_fetch::<SaveDatabase>().unwrap();
        self.database_init(&db.0).unwrap();

        self.attach_touch(world, this);
        self.attach_autosave(world, this);
        self.attach_render(world, this);
    }
}

impl Drop for StrokeLayer {
    fn drop(&mut self) {
        self.thread_tx.send(ThreadInput::Finish).unwrap();
        let thread = self.thread.take().unwrap();
        thread.join().unwrap();
    }
}
