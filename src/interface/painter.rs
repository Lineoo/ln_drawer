use std::borrow::Cow;
use std::sync::Arc;

use parking_lot::Mutex;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

pub struct PainterPipeline {
    pipeline: RenderPipeline,
    painters: Vec<Arc<Painter>>,
}
impl PainterPipeline {
    pub fn init(device: &Device, surface: &SurfaceConfiguration) -> PainterPipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("painter_shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("painter.wgsl"))),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("painter_bind_group_layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("painter_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("painter_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                            shader_location: 1,
                            format: VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(surface.format.into())],
            }),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        PainterPipeline {
            painters: Vec::new(),
            pipeline,
        }
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        for painter in &self.painters {
            if Arc::strong_count(painter) > 1 {
                painter.render(rpass);
            }
        }
    }

    pub fn create(&mut self, rect: [f32; 4], width: u32, height: u32, device: &Device) -> Arc<Painter> {
        self.clean();
        let painter = Arc::new(Painter::init(rect, width, height, device));
        self.painters.push(painter.clone());
        painter
    }

    pub fn clean(&mut self) {
        self.painters
            .retain(|painter| Arc::strong_count(painter) > 1);
    }
}

pub struct Painter {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    bind_texture: Texture,

    width: u32,
    height: u32,

    buffer: Mutex<Vec<u8>>,
}
impl Painter {
    pub fn init(rect: [f32; 4], width: u32, height: u32, device: &Device) -> Painter {
        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_vertex_buffer"),
            contents: bytemuck::bytes_of(&[
                Vertex {
                    pos: [rect[0], rect[1]],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: [rect[0], rect[3]],
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: [rect[2], rect[3]],
                    uv: [1.0, 1.0],
                },
                Vertex {
                    pos: [rect[2], rect[1]],
                    uv: [1.0, 0.0],
                },
            ]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_index_buffer"),
            contents: bytemuck::bytes_of(&[0, 1, 2, 0, 3, 2]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        let buffer = vec![0; (width * height * 4) as usize];

        let bind_texture = device.create_texture(&TextureDescriptor {
            label: Some("painter_texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let bind_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("painter_sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("painter_bind_group_layout"),
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

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("painter_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &bind_texture.create_view(&Default::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&bind_sampler),
                },
            ],
        });

        Painter {
            vertices,
            indices,
            bind_group,
            bind_texture,
            buffer: Mutex::new(buffer),
            width,
            height,
        }
    }

    pub fn set_pixel(&self, x: u32, y: u32, color: [u8; 4]) {
        let mut buffer = self.buffer.lock();
        let start = (x.rem_euclid(self.width) + y.rem_euclid(self.height) * self.width) * 4;
        let start = start as usize;

        buffer[start] = color[0];
        buffer[start + 1] = color[1];
        buffer[start + 2] = color[2];
        buffer[start + 3] = color[3];
    }

    pub fn flush(&self, queue: &Queue) {
        let buffer = self.buffer.lock();
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.bind_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &buffer,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn render(&self, rpass: &mut RenderPass) {
        rpass.set_bind_group(0, Some(&self.bind_group), &[]);
        rpass.set_vertex_buffer(0, self.vertices.slice(..));
        rpass.set_index_buffer(self.indices.slice(..), IndexFormat::Uint32);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}
