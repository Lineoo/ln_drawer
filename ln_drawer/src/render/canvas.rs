use ln_world::{Descriptor, Element, Handle, World};
use palette::{Srgba, blend::Compose};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, Extent3d, FragmentState,
    Origin3d, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::TextureDataOrder,
};

use crate::{
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{Camera, CameraBind},
        vertex::VertexUniform,
    },
};

pub struct Canvas {
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,

    data: Vec<u8>,
    width: u32,
    height: u32,

    control: Handle<RenderControl>,
    vertex: Buffer,
    texture: Texture,
    queue: Queue,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CanvasDescriptor {
    pub data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,
}

pub struct CanvasManager {
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct CanvasManagerDescriptor;

impl Descriptor for CanvasManagerDescriptor {
    type Target = Handle<CanvasManager>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();

        let shader_vs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: ShaderSource::Wgsl(include_str!("vertex.wgsl").into()),
        });

        let shader_fs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("canvas_shader"),
            source: ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });

        let bind_layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("canvas_bind_layout"),
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
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("canvas_pipeline_layout"),
                bind_group_layouts: &[&camera.layout, &bind_layout],
                immediate_size: 0,
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("canvas_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader_vs,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                fragment: Some(FragmentState {
                    module: &shader_fs,
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

        world.insert(CanvasManager {
            pipeline,
            bind_layout,
        })
    }
}

impl Element for CanvasManager {}

impl Descriptor for CanvasDescriptor {
    type Target = Handle<Canvas>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let manager = &mut *world.single_fetch_mut::<CanvasManager>().unwrap();

        // instance //

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("canvas_uniform"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: self.rect.origin.into_array(),
                extend: self.rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let desc = TextureDescriptor {
            label: Some("canvas_texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::COPY_DST
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        };

        let texture = match &self.data {
            Some(data) => {
                assert_eq!(
                    data.len(),
                    (self.width * self.height) as usize * 4,
                    "data is not matched with its size"
                );
                render.device.create_texture_with_data(
                    &render.queue,
                    &desc,
                    TextureDataOrder::LayerMajor,
                    data,
                )
            }
            None => render.device.create_texture(&desc),
        };

        let sampler = render.device.create_sampler(&SamplerDescriptor {
            label: Some("canvas"),
            ..Default::default()
        });

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("canvas_texture_view"),
            ..Default::default()
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas"),
            layout: &manager.bind_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &uniform,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass| {
                let manager = world.single_fetch::<CanvasManager>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                rpass.set_pipeline(&manager.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &bind, &[]);
                rpass.draw(0..4, 0..1);
            })),
        });

        world.insert(Canvas {
            data: match self.data {
                Some(bytes) => bytes.to_vec(),
                None => vec![0; (self.rect.width() * self.rect.height()) as usize * 4],
            },
            width: self.width,
            height: self.height,
            rect: self.rect,
            order: self.order,
            visible: self.visible,
            control,
            vertex: uniform,
            texture,
            queue: render.queue.clone(),
        })
    }
}

impl Element for Canvas {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        RenderControl::reorder(self.visible.then_some(self.order), world, self.control);
        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        RenderControl::reorder(self.visible.then_some(self.order), world, self.control);

        let uniform = VertexUniform {
            origin: self.rect.origin.into_array(),
            extend: self.rect.extend.into_array(),
        };

        let bytes = bytemuck::bytes_of(&uniform);
        self.queue.write_buffer(&self.vertex, 0, bytes);
    }
}

impl CanvasDescriptor {
    pub fn from_bytes(
        rect: Rectangle,
        order: isize,
        bytes: &[u8],
    ) -> Result<CanvasDescriptor, Box<dyn std::error::Error>> {
        let image = image::load_from_memory(bytes)?;
        Ok(CanvasDescriptor {
            data: Some(image.as_bytes().to_vec()),
            width: image.width(),
            height: image.height(),
            rect,
            order,
            visible: true,
        })
    }
}

impl Canvas {
    pub fn to_descriptor(&self) -> CanvasDescriptor {
        CanvasDescriptor {
            data: Some(self.data.clone()),
            width: self.width,
            height: self.height,
            rect: self.rect,
            order: self.order,
            visible: self.visible,
        }
    }

    pub fn open_writer(&mut self) -> CanvasWriter<'_> {
        CanvasWriter { canvas: self }
    }

    pub fn read(&self, x: i32, y: i32) -> Srgba {
        let x = x.rem_euclid(self.width as i32);
        let y = y.rem_euclid(self.height as i32);

        let start = ((x + y * self.width as i32) * 4) as usize;
        let data = &self.data;

        Srgba::new(
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        )
        .into_format()
    }

    pub fn write(&mut self, x: i32, y: i32, color: Srgba) {
        let x = x.rem_euclid(self.width as i32);
        let y = y.rem_euclid(self.height as i32);

        let start = ((x + y * self.width as i32) * 4) as usize;
        let data = &mut self.data;

        let color = Srgba::<u8>::from_format(color);
        data[start] = color.red;
        data[start + 1] = color.green;
        data[start + 2] = color.blue;
        data[start + 3] = color.alpha;

        let data = self.data[start..start + 4].to_vec();
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: x as u32,
                    y: y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &data,
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

    pub fn draw(&mut self, x: i32, y: i32, color: Srgba) {
        let prev = self.read(x, y);
        let next = color.over(prev);
        self.write(x, y, next);
    }
}

/// A more efficient way to write lots of data into canvas' Buffer
pub struct CanvasWriter<'painter> {
    pub canvas: &'painter mut Canvas,
}

impl CanvasWriter<'_> {
    pub fn read(&self, x: i32, y: i32) -> Srgba {
        let x = x.rem_euclid(self.canvas.width as i32);
        let y = y.rem_euclid(self.canvas.height as i32);

        let start = ((x + y * self.canvas.width as i32) * 4) as usize;
        let data = &self.canvas.data;

        Srgba::new(
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        )
        .into_format()
    }

    pub fn write(&mut self, x: i32, y: i32, color: Srgba) {
        let x = x.rem_euclid(self.canvas.width as i32);
        let y = y.rem_euclid(self.canvas.height as i32);

        let start = ((x + y * self.canvas.width as i32) * 4) as usize;
        let data = &mut self.canvas.data;

        let color = Srgba::<u8>::from_format(color);
        data[start] = color.red;
        data[start + 1] = color.green;
        data[start + 2] = color.blue;
        data[start + 3] = color.alpha;
    }

    pub fn draw(&mut self, x: i32, y: i32, color: Srgba) {
        let prev = self.read(x, y);
        let next = color.over(prev);
        self.write(x, y, next);
    }
}

impl Drop for CanvasWriter<'_> {
    fn drop(&mut self) {
        let rect = self.canvas.rect;
        let data = self.canvas.data.clone();

        self.canvas.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.canvas.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &data,
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
