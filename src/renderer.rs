use std::{borrow::Cow, sync::Arc};

use wgpu::{
    wgt::{SamplerDescriptor, TextureViewDescriptor},
    *,
};
use winit::window::Window;

use crate::layout::canvas::Canvas;

pub struct LnDrawerRenderer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    pipeline: RenderPipeline,

    width: u32,
    height: u32,

    drawing_bind_group: BindGroup,

    drawing_buffer: Vec<u8>,
    drawing_texture: Texture,

    canvas: Canvas,
}

impl LnDrawerRenderer {
    pub async fn new(window: Arc<Window>) -> LnDrawerRenderer {
        let instance = Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::defaults(),
                memory_hints: MemoryHints::MemoryUsage,
                trace: Trace::Off,
            })
            .await
            .unwrap();

        // Surface Configuration
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_config = surface.get_default_config(&adapter, width, height).unwrap();

        surface.configure(&device, &surface_config);

        // Drawing
        let drawing_texture = device.create_texture(&TextureDescriptor {
            label: None,
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

        let drawing_buffer = vec![0; (width * height * 4) as usize];

        let drawing_view = drawing_texture.create_view(&TextureViewDescriptor::default());
        let drawing_sampler = device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let drawing_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2,
                            sample_type: TextureSampleType::Float { filterable: true },
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

        let drawing_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &drawing_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&drawing_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&drawing_sampler),
                },
            ],
        });

        // Shader & Pipeline
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("main_pipeline_layout"),
            bind_group_layouts: &[&drawing_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("main_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(surface_config.format.into())],
            }),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // write image
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &drawing_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &drawing_buffer,
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

        // Canvas
        let mut canvas = Canvas::default();
        canvas.new_instance(0.0, 0.3, 0.5, 0.0);
        canvas.setup(&device);

        LnDrawerRenderer {
            surface,
            device,
            queue,
            pipeline,
            width,
            height,
            drawing_bind_group,
            drawing_buffer,
            drawing_texture,
            canvas
        }
    }

    pub fn write_buffer(&mut self) {
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.drawing_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &self.drawing_buffer,
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

    pub fn redraw(&mut self) {
        let texture = self.surface.get_current_texture().unwrap();

        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&self.drawing_bind_group), &[]);
        pass.draw(0..3, 0..1);

        self.canvas.render(&mut pass);

        drop(pass);

        self.queue.submit([encoder.finish()]);

        texture.present();
    }

    pub fn brush(&mut self, x: i32, y: i32) {
        let x = x.rem_euclid(self.width as i32);
        let y = y.rem_euclid(self.height as i32);
        let start = (x + y * self.width as i32) as usize * 4;

        self.drawing_buffer[start] = 255;
        self.drawing_buffer[start + 1] = 255;
        self.drawing_buffer[start + 2] = 255;
        self.drawing_buffer[start + 3] = 255;
    }
}
