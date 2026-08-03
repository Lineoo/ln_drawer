use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, World};
use palette::{Srgba, blend::Compose};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, Extent3d, FilterMode,
    FragmentState, Origin3d, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension,
    VertexState,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::TextureDataOrder,
};

use crate::{
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{Camera, CameraBind},
        rectangle::RectangleUniform,
    },
    widgets::{SetWidgetRectangle, SetWidgetVisible},
};

pub struct Canvas {
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,
    pub color: Srgba,

    pub data: Vec<u8>,
    pub data_width: u32,
    pub data_height: u32,
}

pub struct CanvasInstance {
    rectangle_uniform: Buffer,
    color_uniform: Buffer,
    bind: BindGroup,
    texture: Texture,
}

pub struct CanvasPipeline {
    pipeline: RenderPipeline,
    instance: BindGroupLayout,
}

pub struct SetCanvasColor(pub Srgba);
pub struct UploadCanvasData;
pub struct RemakeCanvasTexture;

impl Canvas {
    pub fn init(&mut self, world: &World, this: Handle<Self>) {
        assert_eq!(
            self.data.len(),
            (self.data_width * self.data_height) as usize * 4,
            "data is not matched with its size"
        );

        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<CanvasPipeline>().unwrap();
        let instance = world.insert(self.instantiate(&render, &pipeline));
        drop(render);

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass, extra| {
                let instance = world.fetch(instance).unwrap();
                let pipeline = world.single_fetch::<CanvasPipeline>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let (start, end) = extra.diagnosis.assign("main > canvas");
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &instance.bind, &[]);
                rpass.draw(0..4, 0..1);

                extra.diagnosis.write(rpass, end);
            })),
        });

        RenderControl::reorder(self.visible.then_some(self.order), world, control);

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let instance = world.fetch(instance).unwrap();
            this.rect = rect;

            let uniform = RectangleUniform {
                origin: rect.origin.into(),
                extend: rect.extend.into(),
            };

            let bytes = bytemuck::bytes_of(&uniform);
            let render = world.single_fetch::<Render>().unwrap();
            render
                .queue
                .write_buffer(&instance.rectangle_uniform, 0, bytes);
        });

        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.visible = visible;
            RenderControl::reorder(this.visible.then_some(this.order), world, control);
        });

        world.observer(this, move |&SetCanvasColor(color), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let instance = world.fetch(instance).unwrap();
            this.color = color;

            let uniform = <[f32; 4]>::from(color.into_linear());

            let bytes = bytemuck::bytes_of(&uniform);
            let render = world.single_fetch::<Render>().unwrap();
            render.queue.write_buffer(&instance.color_uniform, 0, bytes);
        });

        world.observer(this, move |&UploadCanvasData, world| {
            let this = world.fetch(this).unwrap();
            let instance = world.fetch(instance).unwrap();
            let render = world.single_fetch::<Render>().unwrap();
            this.upload_full(&instance, &render.queue);
        });

        world.observer(this, move |&RemakeCanvasTexture, world| {
            let this = world.fetch(this).unwrap();
            let mut instance = world.fetch_mut(instance).unwrap();
            let pipeline = world.single_fetch::<CanvasPipeline>().unwrap();
            let render = world.single_fetch::<Render>().unwrap();
            *instance = this.instantiate(&render, &pipeline)
        });
    }

    pub fn transparent(rect: Rectangle, order: isize, visible: bool, size: UVec2) -> Self {
        Self {
            rect,
            order,
            visible,
            color: Srgba::new(1.0, 1.0, 1.0, 1.0),
            data: vec![0u8; (size.x * size.y * 4) as usize],
            data_width: size.x,
            data_height: size.y,
        }
    }

    pub fn read(&self, x: i32, y: i32) -> Srgba {
        let start = self.offset(x, y);
        if start + 3 >= self.data.len() {
            return Srgba::new(0.0, 0.0, 0.0, 0.0);
        }

        Srgba::new(
            self.data[start],
            self.data[start + 1],
            self.data[start + 2],
            self.data[start + 3],
        )
        .into_format()
    }

    pub fn write(&mut self, x: i32, y: i32, color: Srgba) {
        let start = self.offset(x, y);
        if start + 3 >= self.data.len() {
            return;
        }

        let color = Srgba::<u8>::from_format(color);
        self.data[start] = color.red;
        self.data[start + 1] = color.green;
        self.data[start + 2] = color.blue;
        self.data[start + 3] = color.alpha;
    }

    pub fn draw_over(&mut self, x: i32, y: i32, color: Srgba) {
        let prev = self.read(x, y);
        let next = color.over(prev);
        self.write(x, y, next);
    }

    pub fn clear_transparent(&mut self) {
        self.data.fill(0);
    }

    pub fn upload(
        &self,
        p: IVec2,
        s: UVec2,
        data: &[u8],
        instance: &CanvasInstance,
        queue: &Queue,
    ) {
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &instance.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: self.offset(p.x, p.y) as u64,
                bytes_per_row: Some(s.x * 4),
                rows_per_image: Some(s.y),
            },
            Extent3d {
                width: s.x,
                height: s.y,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn upload_full(&self, instance: &CanvasInstance, queue: &Queue) {
        self.upload(
            IVec2::ZERO,
            UVec2::new(self.data_width, self.data_height),
            &self.data,
            instance,
            queue,
        );
    }

    fn offset(&self, x: i32, y: i32) -> usize {
        ((x + y * self.data_width as i32) * 4) as usize
    }

    fn instantiate(&self, render: &Render, pipeline: &CanvasPipeline) -> CanvasInstance {
        let rectangle_uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("canvas_uniform"),
            contents: bytemuck::bytes_of(&RectangleUniform {
                origin: self.rect.origin.into(),
                extend: self.rect.extend.into(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let desc = TextureDescriptor {
            label: Some("canvas_texture"),
            size: Extent3d {
                width: self.data_width,
                height: self.data_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = render.device.create_texture_with_data(
            &render.queue,
            &desc,
            TextureDataOrder::LayerMajor,
            &self.data,
        );

        let sampler = render.device.create_sampler(&SamplerDescriptor {
            label: Some("canvas"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("canvas_texture_view"),
            ..Default::default()
        });

        let color_uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("canvas_uniform"),
            contents: bytemuck::bytes_of(&<[f32; 4]>::from(self.color)),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("canvas"),
            layout: &pipeline.instance,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &rectangle_uniform,
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
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &color_uniform,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        CanvasInstance {
            rectangle_uniform,
            color_uniform,
            bind,
            texture,
        }
    }
}

impl CanvasPipeline {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();

        let shader = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("canvas_shader"),
            source: ShaderSource::Wgsl(
                format!(
                    "{}{}{}",
                    include_str!("lib_camera.wgsl"),
                    include_str!("lib_rectangle.wgsl"),
                    include_str!("canvas.wgsl"),
                )
                .into(),
            ),
        });

        let instance = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("canvas_bind_layout"),
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
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("canvas_pipeline_layout"),
                bind_group_layouts: &[Some(&camera.layout), Some(&instance)],
                immediate_size: 0,
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("canvas_pipeline"),
                layout: Some(&pipeline_layout),
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

        CanvasPipeline { pipeline, instance }
    }
}

impl Element for Canvas {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for CanvasInstance {}
impl Element for CanvasPipeline {}
