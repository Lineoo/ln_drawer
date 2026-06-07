pub mod dirty;
pub mod interpolate;
pub mod modifier;
pub mod shape;
mod stream;

use std::{
    sync::mpsc::{Receiver, Sender, channel},
    thread::JoinHandle,
};

use bytemuck::{bytes_of, cast_slice};
use glam::Vec2;
use hashbrown::{HashMap, HashSet};
use ln_world::{Element, Handle, World};
use palette::Srgba;
use redb::{Database, ReadableDatabase, TableDefinition};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, FilterMode, FragmentState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};
use winit::event::PointerKind;

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position, PositionFract, Rectangle, Size},
    render::{
        COMPOSITING_FORMAT, MSAA_STATE, Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged, CameraUtils, UICamera},
        rounded::{RoundedRect, RoundedRectDescriptor},
        vertex::VertexUniform,
    },
    save::{Autosave, SaveDatabase},
    stroke::{
        dirty::Dirty,
        interpolate::{Draw, Interpolation},
        modifier::{DrawProcessed, DrawProcessedStorage, Modifier},
    },
    tools::{
        collider::ToolCollider,
        pointer::{PointerHover, PointerHoverStatus},
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    widgets::{WidgetEnabled, WidgetRectangle},
};

const CHUNK_SIZE: u32 = 512;
const CHUNK_CAPS: usize = 512;
const CHUNK_BATCH: usize = 8;
const CHUNK_MIPMAP: u8 = 8;
const MAX_STROKE: u64 = 200;

const CHUNK_META0_FORMAT: u32 = 1;

const TABLE_STROKE_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
    TableDefinition::new("stroke_chunk");
const TABLE_STROKE_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
    TableDefinition::new("stroke_chunk_meta");

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
    meta_unsaved: HashSet<ChunkKey>,

    pub render_debugging: bool,
    render_pipeline: RenderPipeline,
    render_debug_pipeline: RenderPipeline,
    render_sampler: Sampler,

    mipmap_pipeline: ComputePipeline,
    gamma_fixing_pipeline: ComputePipeline,
    brush_round_pipeline: ComputePipeline,

    chunk_render_layout: BindGroupLayout,
    chunk_draw_layout: BindGroupLayout,

    dispatch: Buffer,
    draws_length: Buffer,
    draws_array: Buffer,

    dispatch_group: BindGroup,
    dispatch_group_draw: BindGroup,

    thread_tx: Sender<ThreadInput>,
    thread_rx: Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,

    brush_preview: Handle<RoundedRect>,

    pub interpolation: Interpolation,
    pub modifier: Modifier,
    pub dirty: Dirty,
    pub shape: u32,
    prev: Option<Draw>,
}

struct Chunk {
    bind: ChunkBind,
    meta0: ChunkMeta0,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ChunkMeta0 {
    format: u32,
    mipmapped: bool,
}

/// There are two `BindGroup`s here in pipelines:
/// - First the __global__ layer
///     - the __dirty rectangle__ in mipmapping & color space compute shader
///     - the __dirty rectangle and strokes__ in painting compute shader
///     - the __camera binding and sampler__ in render shader
/// - Second the __chunk__ layer
///     - chunk_key bind contains TextureView and `vec3i` chunk_key
///     - a additional sampler is included in render
///     - For obvious reason the mipmap binding has two sets of data.
///         - Maybe we could get a third bind group for that
struct ChunkBind {
    texture: Texture,
    render: BindGroup,
    draw: BindGroup,
}

enum ThreadInput {
    SetStreamCamera(Fract, Size, PositionFract),
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
    dispatch_coords: [i32; 2],
    dispatch_size: [u32; 2],
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

        let chunk_render_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("chunk_render"),
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

        let chunk_draw_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("chunk_draw"),
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

        let dispatch_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("dispatch"),
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

        let dispatch_group_draw_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("dispatch_draw"),
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
            });

        let dispatch = device.create_buffer(&BufferDescriptor {
            label: Some("dispatch"),
            size: size_of::<DispatchUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_length = device.create_buffer(&BufferDescriptor {
            label: Some("draws_length"),
            size: size_of::<u32>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_array = device.create_buffer(&BufferDescriptor {
            label: Some("draws_array"),
            size: size_of::<DrawProcessedStorage>() as u64 * MAX_STROKE,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dispatch_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("dispatch"),
            layout: &dispatch_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &dispatch,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let dispatch_group_draw = device.create_bind_group(&BindGroupDescriptor {
            label: Some("dispatch"),
            layout: &dispatch_group_draw_layout,
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

        let render_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("stroke_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("stroke_chunk"),
            source: ShaderSource::Wgsl(include_str!("stroke/chunk.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("stroke_chunk"),
            bind_group_layouts: &[&camera_bind.layout, &chunk_render_layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("stroke_chunk"),
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
                    format: COMPOSITING_FORMAT,
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
            label: Some("stroke_chunk_debug"),
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
                    format: COMPOSITING_FORMAT,
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        });

        let mipmap_pipeline = mipmap_pipeline(device, &chunk_draw_layout, &dispatch_group_layout);
        let gamma_fixing_pipeline =
            gamma_fixing_pipeline(device, &chunk_draw_layout, &dispatch_group_layout);
        let brush_round_pipeline =
            shape::brush_round(&render, &dispatch_group_draw_layout, &chunk_draw_layout);

        let (thread_input_tx, thread_input_rx) = channel();
        let (thread_output_tx, thread_output_rx) = channel();

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();
        let camera = world.single_fetch::<Camera>().unwrap();
        let render = world.single_fetch::<Render>().unwrap();
        let device = render.device.clone();
        let queue = render.queue.clone();

        thread_input_tx
            .send(ThreadInput::SetStreamCamera(
                camera.zoom,
                camera.size,
                camera.center,
            ))
            .unwrap();

        let thread = std::thread::spawn(|| {
            stream::loading_thread(database, device, queue, thread_input_rx, thread_output_tx)
                .unwrap();
        });

        let ui_camera = world.single_fetch::<UICamera>().unwrap();
        let brush_preview = world.enter(ui_camera.0, || {
            world.build(RoundedRectDescriptor {
                rect: Rectangle::new_half(Position::new(0, 0), Size::new(5, 5)),
                color: Srgba::new(0.5, 0.5, 0.5, 0.4),
                shrink: 8.0,
                value: 8.0,
                shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.2),
                shadow_offset: Vec2::ZERO,
                shadow_blur: 30.0,
                visible: false,
                vertex_extend: 80,
                order: -10,
                ..Default::default()
            })
        });

        StrokeLayer {
            chunks: HashMap::new(),
            meta_unsaved: HashSet::new(),
            render_debugging: false,
            render_pipeline,
            render_debug_pipeline,
            render_sampler,
            mipmap_pipeline,
            gamma_fixing_pipeline,
            brush_round_pipeline,
            chunk_render_layout,
            chunk_draw_layout,
            dispatch,
            draws_length,
            draws_array,
            dispatch_group,
            dispatch_group_draw,
            thread_tx: thread_input_tx,
            thread_rx: thread_output_rx,
            thread: Some(thread),
            brush_preview,
            interpolation: DEFAULT_INTERPOLATION,
            modifier: DEFAULT_MODIFIER,
            dirty: DEFAULT_DIRTY,
            shape: 0,
            prev: None,
        }
    }

    fn create_chunk(&self, device: &Device, key: ChunkKey) -> ChunkBind {
        let texture = device.create_texture(&chunk_texture_desc());

        Self::create_chunk_from_texture(
            &self.chunk_render_layout,
            &self.chunk_draw_layout,
            &self.render_sampler,
            texture,
            device,
            key,
        )
    }

    /// By only two way you should call this function:
    /// - First you are processing output from stream loading thread by `ThreadOutput::Insert`
    ///     - The main thread may also needs to migrate canvas format.
    /// - Or it could be while drawing and you needs to extend the canvas, in
    ///   which case you should sync with loading thread by `ThreadInput::Create`
    fn create_chunk_from_texture(
        render_layout: &BindGroupLayout,
        draw_layout: &BindGroupLayout,
        render_sampler: &Sampler,
        texture: Texture,
        device: &Device,
        key: ChunkKey,
    ) -> ChunkBind {
        let rect = chunk_rect(key);
        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("stroke_chunk_rectangle"),
            contents: bytes_of(&VertexUniform {
                origin: rect.origin.into_array(),
                extend: rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let key = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("stroke_chunk_key"),
            contents: bytes_of(&ChunkUniform {
                chunk: [key.0, key.1, key.2 as i32],
                _pad: 0,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let texture_fragment_view = texture.create_view(&TextureViewDescriptor {
            label: Some("stroke_chunk_texture_view"),
            format: Some(TextureFormat::Rgba8UnormSrgb),
            usage: Some(TextureUsages::TEXTURE_BINDING),
            ..Default::default()
        });

        let texture_compute_view = texture.create_view(&TextureViewDescriptor {
            label: Some("stroke_chunk_texture_view"),
            format: Some(TextureFormat::Rgba8Unorm),
            usage: Some(TextureUsages::STORAGE_BINDING),
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
                    resource: BindingResource::TextureView(&texture_fragment_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&render_sampler),
                },
            ],
        });

        let draw_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("stroke_chunk_draw"),
            layout: &draw_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_compute_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &key,
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

    fn database_init(&mut self, db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        write.open_table(TABLE_STROKE_CHUNK)?;
        write.commit()?;
        Ok(())
    }

    fn attach_autosave(&mut self, world: &World, this: Handle<Self>) {
        let save = world.insert(Autosave(Box::new(move |world, write| {
            let this = &mut *world.fetch_mut(this).unwrap();
            let mut table_meta = write.open_table(TABLE_STROKE_CHUNK_META).unwrap();
            for key in this.meta_unsaved.drain() {
                let chunk = this.chunks.get(&key).unwrap();
                let meta0 = chunk.as_ref().unwrap().meta0;
                let mut bytes = [0u8; 16];
                postcard::to_slice(&meta0, &mut bytes).unwrap();
                table_meta.insert(((0, key), 0), &bytes[..]).unwrap();
            }
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let camera = world.single::<Camera>().unwrap();
        world.observer(camera, move |_: &CameraPositionChanged, world| {
            let this = world.fetch(this).unwrap();
            let camera = world.fetch(camera).unwrap();

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.size,
                    camera.center,
                ))
                .unwrap();
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
                let mipmap = mipmap_of(camera.zoom);
                let (chunk_src, chunk_dst) = chunks_within(view_rect, mipmap);

                match stroke.render_debugging {
                    false => rpass.set_pipeline(&stroke.render_pipeline),
                    true => rpass.set_pipeline(&stroke.render_debug_pipeline),
                }
                rpass.set_bind_group(0, &camera.bind, &[]);

                for chunk_x in chunk_src.0..chunk_dst.0 {
                    for chunk_y in chunk_src.1..chunk_dst.1 {
                        if let Some(Some(chunk)) = stroke.chunks.get(&(chunk_x, chunk_y, mipmap)) {
                            rpass.set_bind_group(1, &chunk.bind.render, &[]);
                            rpass.draw(0..4, 0..1);
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
                debug_assert!(!self.chunks.contains_key(&chunk_id));

                let mut need_mipmap_fix = false;
                let mut migrated = false;
                let render = world.single_fetch::<Render>().unwrap();
                let chunk = texture.and_then(|texture| {
                    let mut new_chunk = Self::create_chunk_from_texture(
                        &self.chunk_render_layout,
                        &self.chunk_draw_layout,
                        &self.render_sampler,
                        texture,
                        &render.device,
                        chunk_id,
                    );

                    let database = world.single_fetch::<SaveDatabase>().unwrap();
                    let read = database.0.begin_read().unwrap();
                    let table_meta = read.open_table(TABLE_STROKE_CHUNK_META).unwrap();
                    let mut meta0 = if let Some(meta0) = table_meta.get(((0, chunk_id), 0)).unwrap()
                    {
                        postcard::from_bytes::<ChunkMeta0>(meta0.value()).unwrap()
                    } else {
                        // __EDGE CASES__: Always expect data, transparent meta data write can be
                        // found in another place while legacy meta data write is in the main database
                        // format migration. If there is texture but no meta, we expect that's a
                        // leftover issues that happens in legacy so we use format 0.
                        log::error!("cannot fetch found chunk meta on {chunk_id:?}!");
                        self.meta_unsaved.insert(chunk_id);
                        ChunkMeta0 {
                            format: 0,
                            mipmapped: true,
                        }
                    };

                    if meta0.format > CHUNK_META0_FORMAT {
                        // We cannot really read this in case we accidentally broke it
                        log::error!(
                            "Cannot read stroke chunk from newer version {:?}",
                            meta0.format
                        );
                        return None;
                    } else if meta0.format < CHUNK_META0_FORMAT {
                        for migrate_format in meta0.format..CHUNK_META0_FORMAT {
                            match migrate_format {
                                0 => {
                                    log::trace!("gamma fixed {chunk_id:?}");
                                    self.fix_gamma(&mut new_chunk, chunk_id, &render);
                                    migrated = true;
                                }
                                _ => unimplemented!("unsupported migration {migrate_format}"),
                            }
                        }
                        meta0.format = CHUNK_META0_FORMAT;
                        self.meta_unsaved.insert(chunk_id);
                    }

                    if !meta0.mipmapped {
                        need_mipmap_fix = true;
                    }

                    Some(Chunk {
                        bind: new_chunk,
                        meta0,
                    })
                });

                self.chunks.insert(chunk_id, chunk);

                if need_mipmap_fix {
                    log::trace!("mipmap fixed {chunk_id:?}");
                    self.fix_unmipmapped(chunk_id, &render);
                }

                if migrated {
                    self.thread_tx
                        .send(ThreadInput::MarkUnsaved(chunk_id))
                        .unwrap();
                }
            }
            ThreadOutput::Remove(key) => {
                debug_assert!(self.chunks.contains_key(&key));

                if self.meta_unsaved.remove(&key) {
                    let database = world.single_fetch::<SaveDatabase>().unwrap();
                    let write = database.0.begin_write().unwrap();
                    let mut table_meta = write.open_table(TABLE_STROKE_CHUNK_META).unwrap();
                    let chunk = self.chunks.get(&key).unwrap();
                    let meta0 = chunk.as_ref().unwrap().meta0;
                    let mut bytes = [0u8; 16];
                    postcard::to_slice(&meta0, &mut bytes).unwrap();
                    table_meta.insert(((0, key), 0), &bytes[..]).unwrap();
                    drop(table_meta);
                    write.commit().unwrap();
                }

                self.chunks.remove(&key);
            }
        }
    }

    fn attach_touch(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider::fullscreen(-100));
        world.dependency(collider, this);

        world.observer(collider, move |event: &PointerHover, world| {
            if let PointerKind::Touch(_) = event.pointer.kind {
                return;
            }

            let this = world.fetch(this).unwrap();
            let ui_camera = world.single_fetch::<UICamera>().unwrap();
            world.enter(ui_camera.0, || {
                let camera = world.single_fetch::<Camera>().unwrap();
                let mut brush_preview = world.fetch_mut(this.brush_preview).unwrap();
                brush_preview.desc.shadow_offset = event.pointer.tilt * 48.0;
                world.queue_trigger(
                    this.brush_preview,
                    WidgetRectangle(Rectangle::new_half(
                        camera
                            .screen_to_world_absolute(event.pointer.screen)
                            .round(),
                        Size::new(5, 5),
                    )),
                );

                match event.status {
                    PointerHoverStatus::Enter => {
                        world.queue_trigger(this.brush_preview, WidgetEnabled(true));
                    }
                    PointerHoverStatus::Moving => {}
                    PointerHoverStatus::Leave => {
                        world.queue_trigger(this.brush_preview, WidgetEnabled(false));
                    }
                }
            });
        });

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

        if !self.validate_chunks(dirty) {
            return;
        }

        // prepare chunks

        let render = world.single_fetch::<Render>().unwrap();
        let mut paint_chunks = Vec::new();
        let mut mipmap_chunks = Vec::new();
        self.prepare_chunks(&render, dirty, &mut paint_chunks, &mut mipmap_chunks);

        // assign works to GPU

        let queue = &render.queue;
        let device = &render.device;

        self.upload_dispatch(dirty, queue);
        self.upload_draws(draw_buf, queue);

        let mut encoder = device.create_command_encoder(&ENCODER_DESC);
        let mut cpass = encoder.begin_compute_pass(&CPASS_DESC);

        cpass.set_pipeline(&self.brush_round_pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch_group_draw), &[]);
        for key in paint_chunks {
            let chunk = self.chunks.get(&key).unwrap();
            let chunk = chunk.as_ref().unwrap();

            cpass.set_bind_group(1, Some(&chunk.bind.draw), &[]);
            cpass_dispatch(dirty, &mut cpass, key);
        }

        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);
        for key in mipmap_chunks {
            let Some(upper) = self.chunks.get(&upper_chunk_of(key)) else {
                continue;
            };
            let upper = upper.as_ref().unwrap();
            let lower = self.chunks.get(&key).unwrap();
            let lower = lower.as_ref().unwrap();

            cpass.set_bind_group(1, Some(&upper.bind.draw), &[]);
            cpass.set_bind_group(2, Some(&lower.bind.draw), &[]);
            cpass_dispatch(dirty, &mut cpass, key);
        }

        drop(cpass);
        queue.submit([encoder.finish()]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }

    fn fix_unmipmapped(&mut self, lower: (i32, i32, u8), render: &Render) {
        let dirty = chunk_rect(lower);

        // prepare chunks

        let mut paint_chunks = Vec::new();
        let mut mipmap_chunks = Vec::new();
        let chunk = self.chunks.get_mut(&lower).unwrap().as_mut().unwrap();
        chunk.meta0.mipmapped = true;
        self.meta_unsaved.insert(lower);
        self.prepare_chunks(render, dirty, &mut paint_chunks, &mut mipmap_chunks);

        // assign works to GPU

        let queue = &render.queue;
        let device = &render.device;

        self.upload_dispatch(dirty, queue);

        let mut encoder = device.create_command_encoder(&ENCODER_DESC);
        let mut cpass = encoder.begin_compute_pass(&CPASS_DESC);

        cpass.set_pipeline(&self.mipmap_pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);
        for key in mipmap_chunks {
            let Some(upper) = self.chunks.get(&upper_chunk_of(key)) else {
                let lower = self.chunks.get_mut(&key).unwrap();
                let lower = lower.as_mut().unwrap();
                lower.meta0.mipmapped = false;
                self.meta_unsaved.insert(key);
                continue;
            };
            let upper = upper.as_ref().unwrap();
            let lower = self.chunks.get(&key).unwrap();
            let lower = lower.as_ref().unwrap();

            cpass.set_bind_group(1, Some(&upper.bind.draw), &[]);
            cpass.set_bind_group(2, Some(&lower.bind.draw), &[]);
            cpass_dispatch(dirty, &mut cpass, upper_chunk_of(key));
        }

        drop(cpass);
        queue.submit([encoder.finish()]);
    }

    fn fix_gamma(&mut self, chunk: &mut ChunkBind, lower: (i32, i32, u8), render: &Render) {
        let dirty = chunk_rect(lower);

        let queue = &render.queue;
        let device = &render.device;

        self.upload_dispatch(dirty, queue);

        let mut encoder = device.create_command_encoder(&ENCODER_DESC);
        let mut cpass = encoder.begin_compute_pass(&CPASS_DESC);

        cpass.set_pipeline(&self.gamma_fixing_pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch_group), &[]);
        cpass.set_bind_group(1, Some(&chunk.draw), &[]);
        cpass_dispatch(chunk_rect(lower), &mut cpass, lower);

        drop(cpass);
        queue.submit([encoder.finish()]);
    }

    fn upload_dispatch(&mut self, dirty: Rectangle, queue: &Queue) {
        let dispatch = DispatchUniform {
            dispatch_coords: dirty.origin.into_array(),
            dispatch_size: dirty.extend.into_array(),
        };
        queue.write_buffer(&self.dispatch, 0, bytes_of(&dispatch));
    }

    fn upload_draws(&mut self, draw_buf: Vec<DrawProcessed>, queue: &Queue) {
        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        queue.write_buffer(&self.draws_length, 0, bytes_of(&(draw_stg.len() as u32)));
        queue.write_buffer(&self.draws_array, 0, cast_slice(&draw_stg));
    }

    fn validate_chunks(&mut self, dirty: Rectangle) -> bool {
        for mipmap in 0..CHUNK_MIPMAP {
            let (chunk_src, chunk_dst) = chunks_within(dirty, mipmap);
            for chunk_x in chunk_src.0..chunk_dst.0 {
                for chunk_y in chunk_src.1..chunk_dst.1 {
                    let chunk_id = (chunk_x, chunk_y, mipmap);

                    if let None = self.chunks.get(&chunk_id) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Assume `validate_chunks` results true.
    fn prepare_chunks(
        &mut self,
        render: &Render,
        dirty: Rectangle,
        paint_chunks: &mut Vec<(i32, i32, u8)>,
        mipmap_chunks: &mut Vec<(i32, i32, u8)>,
    ) {
        for mipmap in 0..CHUNK_MIPMAP {
            let (chunk_src, chunk_dst) = chunks_within(dirty, mipmap);
            for chunk_x in chunk_src.0..chunk_dst.0 {
                for chunk_y in chunk_src.1..chunk_dst.1 {
                    let key = (chunk_x, chunk_y, mipmap);

                    if let Some(chunk) = self.chunks.get(&key) {
                        if chunk.is_none() {
                            let bind = self.create_chunk(&render.device, key);

                            self.thread_tx
                                .send(ThreadInput::Create(key, bind.texture.clone()))
                                .unwrap();

                            let chunk = self.chunks.get_mut(&key).unwrap();
                            self.meta_unsaved.insert(key);
                            *chunk = Some(Chunk {
                                bind,
                                meta0: ChunkMeta0 {
                                    format: CHUNK_META0_FORMAT,
                                    // Pure transparent can be seen as mipmapped
                                    mipmapped: true,
                                },
                            });
                        }

                        self.thread_tx.send(ThreadInput::MarkUnsaved(key)).unwrap();

                        mipmap_chunks.push(key);
                        if mipmap == 0 {
                            paint_chunks.push(key);
                        }
                    }
                }
            }
        }
    }
}

fn mipmap_pipeline(
    device: &Device,
    chunk_draw_layout: &BindGroupLayout,
    dispatch_group_layout: &BindGroupLayout,
) -> ComputePipeline {
    let mipmap_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("stroke_chunk"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("stroke/lib_dispatch.wgsl"),
                include_str!("stroke/lib_colorspace.wgsl"),
                include_str!("stroke/mipmap.wgsl")
            )
            .into(),
        ),
    });

    let mipmap_pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("stroke_mipmap"),
        bind_group_layouts: &[dispatch_group_layout, chunk_draw_layout, chunk_draw_layout],
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
    mipmap_pipeline
}

fn gamma_fixing_pipeline(
    device: &Device,
    chunk_draw_layout: &BindGroupLayout,
    dispatch_group_layout: &BindGroupLayout,
) -> ComputePipeline {
    let gamma_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("fix_gamma"),
        source: ShaderSource::Wgsl(
            format!(
                "{}{}{}",
                include_str!("stroke/lib_colorspace.wgsl"),
                include_str!("stroke/lib_dispatch.wgsl"),
                include_str!("stroke/legacy/gamma_fix.wgsl")
            )
            .into(),
        ),
    });

    let gamma_fixing_pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("stroke_mipmap"),
        bind_group_layouts: &[dispatch_group_layout, chunk_draw_layout],
        immediate_size: 0,
    });

    let gamma_fixing_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("fix_gamma"),
        layout: Some(&gamma_fixing_pipeline),
        module: &gamma_shader,
        entry_point: Some("cs_main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None,
    });
    gamma_fixing_pipeline
}

const ENCODER_DESC: CommandEncoderDescriptor<'_> = CommandEncoderDescriptor {
    label: Some("stroke"),
};

const CPASS_DESC: ComputePassDescriptor<'_> = ComputePassDescriptor {
    label: Some("stroke"),
    timestamp_writes: None,
};

fn cpass_dispatch(dirty: Rectangle, cpass: &mut ComputePass, key: (i32, i32, u8)) {
    const WORKGROUP_SIZE: Size = Size::new(16, 16);
    cpass.dispatch_workgroups(
        (dirty.extend.w - 1) / 2u32.pow(key.2 as u32) / WORKGROUP_SIZE.w + 1,
        (dirty.extend.h - 1) / 2u32.pow(key.2 as u32) / WORKGROUP_SIZE.h + 1,
        1,
    );
}

fn chunk_texture_desc() -> TextureDescriptor<'static> {
    TextureDescriptor {
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
        view_formats: &[TextureFormat::Rgba8UnormSrgb],
    }
}

fn chunk_rect(key: (i32, i32, u8)) -> Rectangle {
    Rectangle {
        origin: Position::new(key.0 * chunk_size(key.2), key.1 * chunk_size(key.2)),
        extend: Size::splat(chunk_size(key.2) as u32),
    }
}

/// Guaranteed assumption: Upper layer is always loaded first
fn chunk_distance(x: i32, y: i32, z: u8, cx: i32, cy: i32, cz: u8) -> u32 {
    let dx = (x * chunk_size_scale(z) + chunk_size_scale(z.saturating_sub(1)))
        - (cx * chunk_size_scale(cz) + chunk_size_scale(cz.saturating_sub(1)));
    let dy = (y * chunk_size_scale(z) + chunk_size_scale(z.saturating_sub(1)))
        - (cy * chunk_size_scale(cz) + chunk_size_scale(cz.saturating_sub(1)));
    let dz = (CHUNK_MIPMAP - z) as i32 * 0x8000;
    dx.unsigned_abs() + dy.unsigned_abs() + dz.unsigned_abs()
}

fn mipmap_of(zoom: Fract) -> u8 {
    (-zoom.round()).max(0) as u8
}

fn chunk_size(mipmap: u8) -> i32 {
    CHUNK_SIZE as i32 * chunk_size_scale(mipmap)
}

fn chunk_size_scale(mipmap: u8) -> i32 {
    2i32.pow(mipmap as u32)
}

fn chunks_within(view_rect: Rectangle, mipmap: u8) -> ((i32, i32), (i32, i32)) {
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

fn upper_chunk_of(chunk: ChunkKey) -> ChunkKey {
    (chunk.0.div_euclid(2), chunk.1.div_euclid(2), chunk.2 + 1)
}

fn chunk_of(center: Position, zoom: Fract) -> ChunkKey {
    (
        center.x.div_euclid(chunk_size(mipmap_of(zoom))),
        center.y.div_euclid(chunk_size(mipmap_of(zoom))),
        mipmap_of(zoom),
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
