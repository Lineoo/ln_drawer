use std::borrow::Cow;
use std::sync::mpsc::{Receiver, Sender, channel};

use hashbrown::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::interface::InterfaceViewport;

pub struct PainterPipeline {
    pipeline: RenderPipeline,
    removal_tx: Sender<usize>,
    removal_rx: Receiver<usize>,
    painters_idx: usize,
    painters: HashMap<usize, PainterBuffer>,
    bind_group_layout: BindGroupLayout,
    viewport_bind: BindGroup,
}
impl PainterPipeline {
    pub fn init(
        device: &Device,
        surface: &SurfaceConfiguration,
        viewport: &InterfaceViewport,
    ) -> PainterPipeline {
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
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let viewport_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("painter_viewport_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let viewport_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("painter_viewport_bind_group"),
            layout: &viewport_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: viewport.buffer(),
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("painter_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, &viewport_layout],
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
                            format: VertexFormat::Sint32x2,
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
                targets: &[Some(ColorTargetState {
                    format: surface.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (removal_tx, removal_rx) = channel();

        PainterPipeline {
            painters_idx: 0,
            painters: HashMap::new(),
            removal_tx,
            removal_rx,
            pipeline,
            bind_group_layout,
            viewport_bind,
        }
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn create(
        &mut self,
        rect: [i32; 4],
        data: Vec<u8>,
        device: &Device,
        queue: &Queue,
    ) -> Painter {
        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_vertex_buffer"),
            contents: bytemuck::bytes_of(&[
                Vertex {
                    pos: [rect[0], rect[1]],
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: [rect[0], rect[3]],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: [rect[2], rect[3]],
                    uv: [1.0, 0.0],
                },
                Vertex {
                    pos: [rect[2], rect[1]],
                    uv: [1.0, 1.0],
                },
            ]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_index_buffer"),
            contents: bytemuck::bytes_of(&[0, 1, 2, 0, 3, 2]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        // Texture Buffer

        let width = (rect[0] - rect[2]).unsigned_abs();
        let height = (rect[1] - rect[3]).unsigned_abs();

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

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("painter_bind_group"),
            layout: &self.bind_group_layout,
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

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &bind_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let painter_buffer = PainterBuffer {
            vertices,
            indices,
            bind_group,
            bind_texture,
        };

        self.painters
            .insert(self.painters_idx, painter_buffer.clone());
        self.painters_idx += 1;

        Painter {
            rect,
            data,
            buffer: painter_buffer.clone(),
            queue: queue.clone(),
            pipeline_remove: self.removal_tx.clone(),
            pipeline_idx: self.painters_idx - 1,
        }
    }

    pub fn clean(&mut self) {
        for idx in self.removal_rx.try_iter() {
            self.painters.remove(&idx);
        }
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(1, Some(&self.viewport_bind), &[]);
        for painter in self.painters.values() {
            rpass.set_bind_group(0, Some(&painter.bind_group), &[]);
            rpass.set_vertex_buffer(0, painter.vertices.slice(..));
            rpass.set_index_buffer(painter.indices.slice(..), IndexFormat::Uint32);
            rpass.draw_indexed(0..6, 0, 0..1);
        }
    }
}

pub struct Painter {
    rect: [i32; 4],
    data: Vec<u8>,

    pipeline_idx: usize,
    pipeline_remove: Sender<usize>,
    queue: Queue,
    buffer: PainterBuffer,
}
impl Drop for Painter {
    fn drop(&mut self) {
        // FIXME: when program terminate
        if let Err(e) = self.pipeline_remove.send(self.pipeline_idx) {
            log::warn!("Dropping Painter: {e}");
        }
    }
}
impl Painter {
    pub fn set_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let width = self.width();
        let height = self.height();

        let x_offset = x - self.rect[0];
        let y_offset = y - self.rect[1];

        let x_clamped = (x_offset).rem_euclid(width as i32) as u32;
        let y_clamped = (height as i32 - 1 - y_offset).rem_euclid(height as i32) as u32;

        let start = (x_clamped + y_clamped * width) * 4;
        let start = start as usize;

        self.data[start] = color[0];
        self.data[start + 1] = color[1];
        self.data[start + 2] = color[2];
        self.data[start + 3] = color[3];

        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.buffer.bind_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: x_clamped,
                    y: y_clamped,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &self.data[start..start + 4],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn start_writer(&mut self) -> PainterWriter<'_> {
        PainterWriter { painter: self }
    }

    pub fn get_rect(&self) -> [i32; 4] {
        self.rect
    }

    pub fn set_rect(&mut self, rect: [i32; 4]) {
        self.rect = rect;
        self.queue.write_buffer(
            &self.buffer.vertices,
            0,
            bytemuck::bytes_of(&[
                Vertex {
                    pos: [rect[0], rect[1]],
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: [rect[0], rect[3]],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: [rect[2], rect[3]],
                    uv: [1.0, 0.0],
                },
                Vertex {
                    pos: [rect[2], rect[1]],
                    uv: [1.0, 1.0],
                },
            ]),
        );
    }

    pub fn get_position(&self) -> [i32; 2] {
        [self.rect[0], self.rect[1]]
    }

    pub fn set_position(&mut self, position: [i32; 2]) {
        let (width, height) = (self.width(), self.height());
        self.set_rect([
            position[0],
            position[1],
            position[0] + width as i32,
            position[1] + height as i32,
        ]);
    }

    fn width(&self) -> u32 {
        (self.rect[0] - self.rect[2]).unsigned_abs()
    }

    fn height(&self) -> u32 {
        (self.rect[1] - self.rect[3]).unsigned_abs()
    }
}

/// A more efficient way to write data into painter's Buffer
pub struct PainterWriter<'painter> {
    painter: &'painter mut Painter,
}
impl Drop for PainterWriter<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}
impl PainterWriter<'_> {
    pub fn set_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let width = self.painter.width();
        let height = self.painter.height();

        let x_offset = x - self.painter.rect[0];
        let y_offset = y - self.painter.rect[1];

        let x_clamped = (x_offset).rem_euclid(width as i32) as u32;
        let y_clamped = (height as i32 - 1 - y_offset).rem_euclid(height as i32) as u32;

        let start = (x_clamped + y_clamped * width) * 4;
        let start = start as usize;

        self.painter.data[start] = color[0];
        self.painter.data[start + 1] = color[1];
        self.painter.data[start + 2] = color[2];
        self.painter.data[start + 3] = color[3];
    }

    pub fn flush(&mut self) {
        self.painter.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.painter.buffer.bind_texture,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
                aspect: TextureAspect::All,
            },
            &self.painter.data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[derive(Clone)]
struct PainterBuffer {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    bind_texture: Texture,
}

// TODO integrate into shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [i32; 2],
    uv: [f32; 2],
}
