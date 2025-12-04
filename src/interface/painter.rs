use std::sync::mpsc::Sender;

use palette::LinSrgba;
use palette::blend::Compose;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::interface::viewport::InterfaceViewport;
use crate::interface::{ComponentCommand, Interface};
use crate::measures::{Position, Rectangle, ZOrder};

pub struct PainterPipeline {
    pipeline: RenderPipeline,
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
            source: ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("painter.wgsl"))),
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
                    buffer: &viewport.buffer,
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
                topology: PrimitiveTopology::TriangleStrip,
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

        PainterPipeline {
            pipeline,
            bind_group_layout,
            viewport_bind,
        }
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn create(
        &mut self,
        rect: Rectangle,
        data: Vec<u8>,
        comp_idx: usize,
        comp_tx: Sender<(usize, ComponentCommand)>,
        device: &Device,
        queue: &Queue,
    ) -> Painter {
        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_vertex_buffer"),
            contents: bytemuck::bytes_of(&[
                Vertex {
                    pos: rect.left_down().into_array(),
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: rect.left_up().into_array(),
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: rect.right_up().into_array(),
                    uv: [1.0, 0.0],
                },
                Vertex {
                    pos: rect.right_down().into_array(),
                    uv: [1.0, 1.0],
                },
            ]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("painter_index_buffer"),
            contents: bytemuck::bytes_of(&[0, 1, 3, 2]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        // Texture Buffer

        let width = rect.width();
        let height = rect.height();

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

        Painter {
            rect,
            z_order: 0,
            width,
            height,
            data,
            comp_idx,
            comp_tx,
            buffer: painter_buffer.clone(),
            queue: queue.clone(),
        }
    }

    pub fn set_pipeline(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(1, Some(&self.viewport_bind), &[]);
    }
}

pub struct Painter {
    rect: Rectangle,
    z_order: isize,

    // Texture
    width: u32,
    height: u32,
    data: Vec<u8>,

    comp_idx: usize,
    comp_tx: Sender<(usize, ComponentCommand)>,

    queue: Queue,
    buffer: PainterBuffer,
}
impl Drop for Painter {
    fn drop(&mut self) {
        // FIXME: when program terminate
        if let Err(e) = (self.comp_tx).send((self.comp_idx, ComponentCommand::Destroy)) {
            log::warn!("Dropping Painter: {e}");
        }
    }
}
impl Painter {
    #[must_use = "The painter will be destroyed when being drop."]
    pub fn new(rect: Rectangle, interface: &mut Interface) -> Painter {
        interface.create_painter(rect)
    }

    #[must_use = "The painter will be destroyed when being drop."]
    pub fn new_with(rect: Rectangle, data: Vec<u8>, interface: &mut Interface) -> Painter {
        interface.create_painter_with(rect, data)
    }

    pub fn open_writer(&mut self) -> PainterWriter<'_> {
        PainterWriter { painter: self }
    }

    pub fn get_pixel(&self, position: Position) -> [u8; 4] {
        let width = self.rect.width();
        let height = self.rect.height();

        let x_offset = position.x - self.rect.origin.x;
        let y_offset = position.y - self.rect.origin.y;

        let x_clamped = (x_offset).rem_euclid(width as i32) as u32;
        let y_clamped = (height as i32 - 1 - y_offset).rem_euclid(height as i32) as u32;

        let start = (x_clamped + y_clamped * width) * 4;
        let start = start as usize;

        [
            self.data[start],
            self.data[start + 1],
            self.data[start + 2],
            self.data[start + 3],
        ]
    }

    pub fn set_pixel(&mut self, position: Position, color: [u8; 4]) {
        let width = self.rect.width();
        let height = self.rect.height();

        let x_offset = position.x - self.rect.origin.x;
        let y_offset = position.y - self.rect.origin.y;

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

    pub fn get_rect(&self) -> Rectangle {
        self.rect
    }

    pub fn set_rect(&mut self, rect: Rectangle) {
        self.rect = rect;
        self.queue.write_buffer(
            &self.buffer.vertices,
            0,
            bytemuck::bytes_of(&[
                Vertex {
                    pos: rect.left_down().into_array(),
                    uv: [0.0, 1.0],
                },
                Vertex {
                    pos: rect.left_up().into_array(),
                    uv: [0.0, 0.0],
                },
                Vertex {
                    pos: rect.right_up().into_array(),
                    uv: [1.0, 0.0],
                },
                Vertex {
                    pos: rect.right_down().into_array(),
                    uv: [1.0, 1.0],
                },
            ]),
        );
    }

    pub fn get_z_order(&self) -> ZOrder {
        ZOrder::new(self.z_order)
    }

    pub fn set_z_order(&mut self, ord: ZOrder) {
        self.z_order = ord.idx;
        if let Err(e) =
            (self.comp_tx).send((self.comp_idx, ComponentCommand::SetZOrder(self.z_order)))
        {
            log::warn!("Set Visibility: {e}");
        }
    }

    pub(super) fn clone_buffer(&self) -> PainterBuffer {
        self.buffer.clone()
    }
}

/// A more efficient way to write data into painter's Buffer
pub struct PainterWriter<'painter> {
    painter: &'painter mut Painter,
}
impl Drop for PainterWriter<'_> {
    fn drop(&mut self) {
        let rect = self.painter.get_rect();
        self.painter.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.painter.buffer.bind_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &self.painter.data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width() * 4),
                rows_per_image: Some(rect.height()),
            },
            Extent3d {
                width: rect.width(),
                height: rect.height(),
                depth_or_array_layers: 1,
            },
        );
    }
}
impl PainterWriter<'_> {
    pub fn read(&self, x: i32, y: i32) -> [u8; 4] {
        let x = x.rem_euclid(self.painter.width as i32);
        let y = y.rem_euclid(self.painter.height as i32);

        let start = ((x + y * self.painter.width as i32) * 4) as usize;
        let data = &self.painter.data;

        [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ]
    }

    pub fn write(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let x = x.rem_euclid(self.painter.width as i32);
        let y = y.rem_euclid(self.painter.height as i32);

        let start = ((x + y * self.painter.width as i32) * 4) as usize;
        let data = &mut self.painter.data;

        data[start] = color[0];
        data[start + 1] = color[1];
        data[start + 2] = color[2];
        data[start + 3] = color[3];
    }

    pub fn draw(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let x = x.rem_euclid(self.painter.width as i32);
        let y = y.rem_euclid(self.painter.height as i32);

        let start = ((x + y * self.painter.width as i32) * 4) as usize;
        let data = &mut self.painter.data;

        let prev = LinSrgba::new(
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        );
        let curr = LinSrgba::new(color[0], color[1], color[2], color[3]);

        let prev: LinSrgba<f32> = prev.into_format();
        let curr: LinSrgba<f32> = curr.into_format();

        let next: LinSrgba<u8> = curr.over(prev).into_format();

        data[start] = next.red;
        data[start + 1] = next.green;
        data[start + 2] = next.blue;
        data[start + 3] = next.alpha;
    }

    pub fn clear(&mut self, color: [u8; 4]) {
        let width = self.painter.width;
        let height = self.painter.height;

        for x in 0..width {
            for y in 0..height {
                let start = ((x + y * width) * 4) as usize;

                self.painter.data[start] = color[0];
                self.painter.data[start + 1] = color[1];
                self.painter.data[start + 2] = color[2];
                self.painter.data[start + 3] = color[3];
            }
        }
    }
}

#[derive(Clone)]
pub struct PainterBuffer {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    bind_texture: Texture,
}
impl PainterBuffer {
    pub fn draw(&self, rpass: &mut RenderPass) {
        rpass.set_bind_group(0, Some(&self.bind_group), &[]);
        rpass.set_vertex_buffer(0, self.vertices.slice(..));
        rpass.set_index_buffer(self.indices.slice(..), IndexFormat::Uint32);
        rpass.draw_indexed(0..4, 0, 0..1);
    }
}

// TODO integrate into shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [i32; 2],
    uv: [f32; 2],
}
