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
    render::{MSAA_STATE, Render, RenderControl, camera::Camera, vertex::VertexUniform},
    stroke::{CHUNK_SIZE, StrokeLayer},
    world::{Element, Handle, World},
};

pub struct StrokeChunk {
    render: Handle<RenderControl>,
    pub compute: BindGroup,
    texture: Texture,
}

pub struct StrokeChunkPipeline {
    pipeline: RenderPipeline,
    pub compute: BindGroupLayout,
    vertex: BindGroupLayout,
    fragment: BindGroupLayout,
}

impl StrokeChunkPipeline {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("stroke_chunk"),
            source: ShaderSource::Wgsl(include_str!("chunk.wgsl").into()),
        });

        let compute = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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

        let vertex = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("stroke_chunk_vertex"),
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
            label: Some("stroke_chunk_fragment"),
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
            label: Some("stroke_chunk"),
            bind_group_layouts: &[&vertex, &fragment],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("stroke_chunk"),
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
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: None,
        });

        StrokeChunkPipeline {
            pipeline,
            compute,
            vertex,
            fragment,
        }
    }
}

impl Element for StrokeChunkPipeline {}

impl StrokeChunk {
    pub fn new(world: &World, chunk: (i32, i32)) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<Camera>().unwrap();
        let manager = world.single_fetch::<StrokeChunkPipeline>().unwrap();
        let device = &render.device;

        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("stroke_chunk_rectangle"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: [chunk.0 * CHUNK_SIZE as i32, chunk.1 * CHUNK_SIZE as i32],
                extend: [CHUNK_SIZE, CHUNK_SIZE],
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

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

        let texture_view = texture.create_view(&TextureViewDescriptor {
            label: Some("stroke_chunk_texture_view"),
            ..Default::default()
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("stroke_chunk_sampler"),
            ..Default::default()
        });

        let compute = device.create_bind_group(&BindGroupDescriptor {
            label: Some("stroke_chunk_compute"),
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
            label: Some("stroke_chunk_vertex"),
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
            label: Some("stroke_chunk_fragment"),
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

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass| {
                let manager = world.single_fetch::<StrokeChunkPipeline>().unwrap();

                rpass.set_pipeline(&manager.pipeline);
                rpass.set_bind_group(0, &vertex, &[]);
                rpass.set_bind_group(1, &fragment, &[]);
                rpass.draw(0..4, 0..1);
            })),
        });

        StrokeChunk {
            render: control,
            compute,
            texture,
        }
    }

    pub fn from_bytes(world: &World, chunk: (i32, i32), bytes: &[u8]) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let canvas = Self::new(world, chunk);
        render.queue.write_texture(
            TexelCopyTextureInfoBase {
                texture: &canvas.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
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

        canvas
    }

    pub fn device_readback(&self, world: &World) -> Vec<u8> {
        let (tx, rx) = std::sync::mpsc::channel();

        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;
        let queue = &render.queue;

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
        rx.recv().unwrap()
    }
}

impl Element for StrokeChunk {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        RenderControl::reorder(Some(-100), world, self.render);
        let layer = world.single::<StrokeLayer>().unwrap();
        world.dependency(this, layer);
        world.dependency(self.render, this);
    }
}
