use serde_bytes::ByteBuf;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Extent3d, FragmentState, MapMode, Origin3d, PipelineLayoutDescriptor,
    PollType, PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    render::{Render, RenderControl, RenderInformation, vertex::VertexUniform, camera::Camera},
    save::{SaveControl, SaveReadClass, Autosave},
    stroke::{CHUNK_SIZE, StrokeLayer},
    world::{Element, Handle, World},
};

pub struct CanvasChunk {
    save: Handle<SaveControl>,
    pub changed: bool,

    pub compute: BindGroup,
    vertex: BindGroup,
    fragment: BindGroup,

    texture: Texture,
}

pub struct CanvasChunkPipeline {
    pipeline: RenderPipeline,
    pub compute: BindGroupLayout,
    vertex: BindGroupLayout,
    fragment: BindGroupLayout,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Archive {
    chunk: (i32, i32),
    bytes: ByteBuf,
}

impl CanvasChunkPipeline {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("canvas_chunk"),
            source: ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });

        let compute = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canvas_chunk_compute"),
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

        let vertex = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canvas_chunk_vertex"),
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
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let fragment = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canvas_chunk_fragment"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("canvas_chunk"),
            bind_group_layouts: &[&vertex, &fragment],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("canvas_chunk"),
            layout: Some(&pipeline),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: render.config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        CanvasChunkPipeline {
            pipeline,
            compute,
            vertex,
            fragment,
        }
    }
}

impl Element for CanvasChunkPipeline {}

impl CanvasChunk {
    pub fn new(world: &World, chunk: (i32, i32), control: Handle<SaveControl>) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<Camera>().unwrap();
        let manager = world.single_fetch::<CanvasChunkPipeline>().unwrap();
        let device = &render.device;

        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("canvas_chunk_rectangle"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: [chunk.0 * CHUNK_SIZE as i32, chunk.1 * CHUNK_SIZE as i32],
                extend: [CHUNK_SIZE, CHUNK_SIZE],
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("canvas_chunk_texture"),
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

        let texture_view = texture.create_view(&TextureViewDescriptor {
            label: Some("canvas_chunk_texture_view"),
            ..Default::default()
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("canvas_chunk_sampler"),
            ..Default::default()
        });

        let compute = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas_chunk_compute"),
            layout: &manager.compute,
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

        let vertex = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas_chunk_vertex"),
            layout: &manager.vertex,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &camera.uniform,
                        offset: 0,
                        size: None,
                    }),
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

        let fragment = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas_chunk_fragment"),
            layout: &manager.fragment,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        CanvasChunk {
            save: control,
            changed: false,
            vertex,
            compute,
            fragment,
            texture,
        }
    }

    pub fn save_write() -> Autosave {
        Autosave(Box::new(|world| {
            let stroke = world.single_fetch::<StrokeLayer>().unwrap();

            let mut canvases = Vec::new();
            let mut tasks = Vec::new();

            for (&chunk, &canvas) in &stroke.chunks {
                let canvas = world.fetch_mut(canvas).unwrap();
                canvases.push((chunk, canvas));
            }

            for (chunk, canvas) in &mut canvases {
                let task = canvas.device_readback(world, *chunk);
                tasks.push(task);
            }

            pollster::block_on(async {
                for task in tasks {
                    task.await;
                }
            })
        }))
    }

    pub fn save_read() -> SaveReadClass {
        SaveReadClass {
            class: "canvas_chunk".into(),
            read: Box::new(|world, control| {
                let stroke = &mut *world.single_fetch_mut::<StrokeLayer>().unwrap();
                let control = world.fetch(control).unwrap();

                let btyes = control.read(world);
                let archive = postcard::from_bytes::<Archive>(&btyes).unwrap();

                let canvas = CanvasChunk::new(world, archive.chunk, control.handle());
                stroke.queue.write_texture(
                    TexelCopyTextureInfoBase {
                        texture: &canvas.texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &archive.bytes,
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

                let canvas = world.insert(canvas);
                let ret = stroke.chunks.insert(archive.chunk, canvas);
                debug_assert!(ret.is_none());
            }),
        }
    }

    async fn device_readback(&mut self, world: &World, chunk: (i32, i32)) {
        let (tx, rx) = std::sync::mpsc::channel();

        if !self.changed {
            return;
        }

        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;
        let queue = &render.queue;

        self.changed = false;

        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("canvas_readback"),
            size: (CHUNK_SIZE * CHUNK_SIZE * 4) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("canvas_readback"),
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfoBase {
                texture: &self.texture,
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

        if let Ok(bytes) = rx.recv() {
            let archive = Archive {
                chunk,
                bytes: bytes.into(),
            };
            let bytes = postcard::to_stdvec(&archive).unwrap();

            let control = world.fetch(self.save).unwrap();
            control.write(world, &bytes);
        }
    }
}

impl Element for CanvasChunk {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(|_| {
                Some(RenderInformation {
                    render_order: -100,
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(move |world, rpass| {
                let manager = world.single_fetch::<CanvasChunkPipeline>().unwrap();
                let this = world.fetch(this).unwrap();

                rpass.set_pipeline(&manager.pipeline);
                rpass.set_bind_group(0, &this.vertex, &[]);
                rpass.set_bind_group(1, &this.fragment, &[]);
                rpass.draw(0..4, 0..1);
            })),
        });

        let layer = world.single::<StrokeLayer>().unwrap();
        world.dependency(this, layer);
        world.dependency(self.save, this);
        world.dependency(control, this);
    }
}
