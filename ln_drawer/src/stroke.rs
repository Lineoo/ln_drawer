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
use redb::{Database, MultimapTableDefinition, ReadableDatabase, TableDefinition};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoderDescriptor, ComputePassDescriptor, Device, Extent3d, FilterMode,
    FragmentState, MapMode, MipmapFilterMode, Origin3d, PipelineLayoutDescriptor, PollType,
    PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, StorageTextureAccess, TexelCopyBufferInfoBase, TexelCopyBufferLayout,
    TexelCopyTextureInfoBase, Texture, TextureAspect, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDimension, VertexState, wgt::TextureDescriptor,
};
use winit::event::PointerKind;

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Position, Rectangle, Size},
    render::{
        MSAA_STATE, Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged, CameraUtils},
    },
    save::{Autosave, SaveDatabase},
    stroke::{
        chunk::{StrokeChunk, StrokeChunkPipeline},
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
const CHUNK_MIPMAP: u8 = 4;
const MAX_STROKE: u64 = 200;

const TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
    MultimapTableDefinition::new("stroke");
const TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> = TableDefinition::new("stroke_chunk");

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
    texture: HashMap<ChunkKey, Option<Texture>>,
    placeholder: Texture,
    sampler: Sampler,

    render_pipeline: RenderPipeline,
    render_layout: BindGroupLayout,
    render_bind: HashMap<ChunkKey, BindGroup>,

    compute_layout: BindGroupLayout,
    compute_bind: HashMap<ChunkKey, BindGroup>,

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

enum ThreadInput {
    SetStreamCenter(ChunkKey),
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
                    visibility: ShaderStages::VERTEX,
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

        let canvas_chunk_pipeline = StrokeChunkPipeline::new(world);
        let pipeline_for_thread = canvas_chunk_pipeline.clone();
        let brush_round =
            RoundBrush::new(&render, &dispatch_layout, &canvas_chunk_pipeline.compute);
        let brush_pixel =
            PixelBrush::new(&render, &dispatch_layout, &canvas_chunk_pipeline.compute);
        world.insert(canvas_chunk_pipeline);

        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("stroke_chunk"),
            source: ShaderSource::Wgsl(include_str!("stroke/chunk.wgsl").into()),
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

        let placeholder = device.create_texture(&TextureDescriptor {
            label: Some("placeholder"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("stroke_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let (thread_input_tx, thread_input_rx) = channel();
        let (thread_output_tx, thread_output_rx) = channel();

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();
        let camera = world.single_fetch::<Camera>().unwrap();
        let camera_uniform = camera.uniform.clone();
        let render = world.single_fetch::<Render>().unwrap();
        let device = render.device.clone();
        let queue = render.queue.clone();

        let chunk_here = chunk_of(camera.center.round());

        thread_input_tx
            .send(ThreadInput::SetStreamCenter(chunk_here))
            .unwrap();

        let thread = std::thread::spawn(|| {
            Self::loading_thread(
                database,
                camera_uniform,
                pipeline_for_thread,
                device,
                queue,
                thread_input_rx,
                thread_output_tx,
            )
            .unwrap();
        });

        StrokeLayer {
            texture: HashMap::new(),
            placeholder,
            sampler,
            render_pipeline,
            render_layout,
            render_bind: HashMap::new(),
            compute_layout,
            compute_bind: HashMap::new(),
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

    fn database_init(&mut self, db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        write.open_multimap_table(TABLE_STROKE)?;
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
                this.thread_tx
                    .send(ThreadInput::SetStreamCenter(chunk_here))
                    .unwrap();
            }
        });
    }

    fn attach_render(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this).unwrap();
                let render = world.single_fetch::<Render>().unwrap();
                for output in this.thread_rx.try_iter() {
                    match output {
                        ThreadOutput::Insert(chunk_id, texture) => {
                            this.texture.insert(chunk_id, texture);
                        }
                        ThreadOutput::Remove(chunk_id) => {
                            this.texture.remove(&chunk_id);
                        }
                    }
                }

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(|world, rpass| {
                let stroke = world.single_fetch::<StrokeLayer>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let view_rect = camera.world_view_rect();
                let chunk_src = (
                    view_rect.left().div_euclid(CHUNK_SIZE as i32),
                    view_rect.down().div_euclid(CHUNK_SIZE as i32),
                );
                let chunk_dst = (
                    (view_rect.right() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
                    (view_rect.up() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
                );

                for chunk_x in chunk_src.0..chunk_dst.0 {
                    for chunk_y in chunk_src.1..chunk_dst.1 {
                        if let Some(Some(chunk)) = stroke.texture.get(&(chunk_x, chunk_y)) {
                            chunk.redraw(world, rpass);
                        }
                    }
                }
            })),
        });
        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
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

    fn draw(&self, world: &World, rpass: &mut RenderPass) {
        let manager = world.single_fetch::<StrokeChunkPipeline>().unwrap();

        rpass.set_pipeline(&manager.pipeline);
        rpass.set_bind_group(0, &self.vertex, &[]);
        rpass.set_bind_group(1, &self.fragment, &[]);
        rpass.draw(0..4, 0..1);
    }

    fn paint(&mut self, next: Draw, world: &World) {
        // generate draws //

        let mut draw_buf = Vec::new();
        let curr = self
            .interpolation
            .interpolate(self.prev, next, &self.modifier, &mut draw_buf);
        self.prev = Some(curr);

        let dirty = self.dirty.compute(curr.position.round(), &draw_buf);
        if dirty.bounding.extend.w == 0 || dirty.bounding.extend.h == 0 {
            return;
        }

        // prepare chunks

        let mut chunks = Vec::new();
        for chunk_x in dirty.chunk_src.0..dirty.chunk_dst.0 {
            for chunk_y in dirty.chunk_src.1..dirty.chunk_dst.1 {
                let chunk_id = (chunk_x, chunk_y);
                if let Some(chunk) = self.texture.get_mut(&chunk_id) {
                    let chunk = chunk
                        .get_or_insert_with(|| {
                            let render = world.single_fetch::<Render>().unwrap();
                            let camera = world.single_fetch::<Camera>().unwrap();
                            let pipeline = world.single_fetch::<StrokeChunkPipeline>().unwrap();
                            StrokeChunk::new(&camera.uniform, &pipeline, &render.device, chunk_id)
                        })
                        .clone();
                    chunks.push(chunk_id);
                    self.thread_tx
                        .send(ThreadInput::Create(chunk_id, chunk))
                        .unwrap();
                    self.thread_tx
                        .send(ThreadInput::MarkUnsaved(chunk_id))
                        .unwrap();
                }
            }
        }

        // assign works to GPU

        let dispatch = DispatchUniform {
            dirty_coords: dirty.bounding.origin.into_array(),
            stroke_count: draw_buf.len() as u32,
            _pad: 0,
        };

        let render = world.single_fetch::<Render>().unwrap();
        let queue = &render.queue;
        let device = &render.device;

        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        queue.write_buffer(&self.dispatch_meta, 0, bytemuck::bytes_of(&dispatch));
        queue.write_buffer(&self.draws_array, 0, bytemuck::cast_slice(&draw_stg));

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("stroke"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("stroke"),
            timestamp_writes: None,
        });

        match self.shape {
            0 => cpass.set_pipeline(&self.brush_round.pipeline),
            1 => cpass.set_pipeline(&self.brush_pixel.pipeline),
            _ => unreachable!(),
        }

        cpass.set_bind_group(0, Some(&self.dispatch), &[]);

        for chunk in chunks {
            let chunk = self.texture.get(&chunk).unwrap().as_ref().unwrap();

            cpass.set_bind_group(1, Some(&chunk.compute), &[]);

            const WORKGROUP_SIZE: Size = Size::new(16, 16);
            cpass.dispatch_workgroups(
                (dirty.bounding.extend.w - 1) / WORKGROUP_SIZE.w + 1,
                (dirty.bounding.extend.h - 1) / WORKGROUP_SIZE.h + 1,
                1,
            );
        }

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }

    fn loading_thread(
        database: SaveDatabase,
        camera_uniform: Buffer,
        pipeline: StrokeChunkPipeline,
        device: Device,
        queue: Queue,
        input_rx: Receiver<ThreadInput>,
        output_tx: Sender<ThreadOutput>,
    ) -> Result<(), Box<dyn Error>> {
        let mut actual = HashMap::<(i32, i32), Option<Texture>>::new();
        let mut unsaved = HashSet::new();

        let mut tasks_buf = IndexSet::with_capacity(400);
        let mut task_frnt = 0;
        let mut task_batch;

        let mut ring = IndexSet::<(i32, i32)>::new();
        let mut frnt = 0;

        let mut filt_load = IndexSet::new();
        let mut filt_unload = IndexSet::new();

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

                    for chunk_x in chunk_center_x - 10..chunk_center_x + 10 {
                        for chunk_y in chunk_center_y - 10..chunk_center_y + 10 {
                            tasks_buf.insert((chunk_x, chunk_y));
                        }
                    }

                    tasks_buf.sort_by_key(|&(x, y)| {
                        let dx = (x - chunk_center_x).unsigned_abs();
                        let dy = (y - chunk_center_y).unsigned_abs();
                        std::cmp::max(dx, dy)
                    });

                    task_frnt = 0;
                }
                Some(ThreadInput::MarkUnsaved(chunk)) => {
                    unsaved.insert(chunk);
                    continue;
                }
                Some(ThreadInput::Create(chunk_id, chunk)) => {
                    actual.insert(chunk_id, Some(chunk));
                    continue;
                }
                Some(ThreadInput::Autosave) => {
                    let write = database.0.begin_write()?;
                    {
                        let mut table = write.open_multimap_table(TABLE_STROKE)?;
                        let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
                        for chunk_id in unsaved.drain() {
                            let Some(Some(chunk)) = actual.get(&chunk_id) else {
                                continue;
                            };

                            let bytes = chunk.device_readback(&device, &queue);
                            let compressed = zstd::encode_all(&bytes[..], 0)?;
                            table.insert((), chunk_id)?;
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
                let mut table = write.open_multimap_table(TABLE_STROKE)?;
                let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;

                if let Some(Some(chunk)) = actual.remove(&chunk_id) {
                    output_tx.send(ThreadOutput::Remove(chunk_id))?;

                    if unsaved.remove(&chunk_id) {
                        let bytes = chunk.device_readback(&device, &queue);
                        let compressed = zstd::encode_all(&bytes[..], 0)?;
                        table.insert((), chunk_id)?;
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

                    actual.insert(chunk_id, Some(texture));
                    output_tx.send(ThreadOutput::Insert(chunk_id, Some(chunk)))?;
                } else {
                    actual.insert(chunk_id, None);
                    output_tx.send(ThreadOutput::Insert(chunk_id, None))?;
                }
            }
        }
    }
}

fn chunk_of(center: Position) -> ChunkKey {
    (
        center.x.div_euclid(CHUNK_SIZE as i32),
        center.y.div_euclid(CHUNK_SIZE as i32),
        0,
    )
}

fn texture_readback(texture: &Texture, device: &Device, queue: &Queue) -> Vec<u8> {
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
            texture: &texture,
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
