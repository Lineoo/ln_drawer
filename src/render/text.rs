use cosmic_text::{Attrs, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, Extent3d, FilterMode,
    FragmentState, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor,
    TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::TextureDataOrder,
};

use crate::{
    measures::Rectangle,
    render::{
        Redraw, Render, RenderActive, RenderControl,
        viewport::{ViewportInstance, ViewportManager},
    },
    world::{Commander, Descriptor, Element, Handle, World},
};

pub struct Text {
    instance: Handle<TextInstance>,
    cmd: Commander,
}

#[derive(Debug)]
pub struct TextDescriptor<'a> {
    pub text: &'a str,
    pub rect: Rectangle,
    pub metrics: Metrics,
    pub order: isize,
    pub visible: bool,
}

pub struct TextManager {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct TextManagerDescriptor;

pub struct TextInstance {
    bind: BindGroup,
    uniform: wgpu::Buffer,
    texture: Texture,
    sampler: Sampler,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextUniform {
    origin: [i32; 2],
    extend: [i32; 2],
}

impl Default for TextDescriptor<'_> {
    fn default() -> Self {
        Self {
            text: Default::default(),
            rect: Rectangle::new(0, 0, 200, 24),
            metrics: Metrics::new(24.0, 20.0),
            order: 100,
            visible: true,
        }
    }
}

impl Element for Text {}
impl Element for TextManager {}
impl Element for TextInstance {}

impl Descriptor for TextManagerDescriptor {
    type Target = TextManager;

    fn build(self, world: &World) -> Self::Target {
        let mut font_system = FontSystem::new();
        let database = font_system.db_mut();

        let sans = include_bytes!("../../fonts/SourceHanSansCN-Regular.otf").to_vec();
        let serif = include_bytes!("../../fonts/SourceHanSerifCN-Regular.otf").to_vec();

        database.load_font_data(sans);
        database.load_font_data(serif);

        let swash_cache = SwashCache::new();

        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single_fetch::<ViewportManager>().unwrap();

        let caps = render.surface.get_capabilities(&render.adapter);
        let format = *caps.formats.first().unwrap();

        let shader_vs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: ShaderSource::Wgsl(include_str!("vertex.wgsl").into()),
        });

        let shader_fs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });

        let bind_layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("text_bind_layout"),
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
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("text_pipeline_layout"),
                bind_group_layouts: &[&viewport.layout, &bind_layout],
                push_constant_ranges: &[],
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("text_pipeline"),
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
                        format,
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });

        TextManager {
            font_system,
            swash_cache,
            pipeline,
            bind_layout,
        }
    }
}

impl Descriptor for TextDescriptor<'_> {
    type Target = Text;

    fn build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single::<ViewportInstance>().unwrap();
        let manager = &mut *world.single_fetch_mut::<TextManager>().unwrap();

        // instance //

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("text_uniform"),
            contents: bytemuck::bytes_of(&TextUniform {
                origin: self.rect.origin.into_array(),
                extend: self.rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let upscale_metrics = self.metrics.scale(4.0);
        let upscale_width = self.rect.width() * 4;
        let upscale_height = self.rect.height() * 4;

        let mut data = vec![0; (upscale_width * upscale_height * 4) as usize];

        let mut buffer = cosmic_text::Buffer::new(&mut manager.font_system, upscale_metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut manager.font_system);

        let attrs = Attrs::new().family(Family::Name("Source Han Sans CN"));
        buffer_borrow.set_size(
            Some(upscale_width as f32),
            Some(upscale_height as f32),
        );
        buffer_borrow.set_text(self.text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);
        buffer_borrow.draw(
            &mut manager.swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * upscale_width as i32) * 4) as usize;
                let rgba = color.as_rgba();
                if start >= data.len() {
                    return;
                }
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        let texture = render.device.create_texture_with_data(
            &render.queue,
            &TextureDescriptor {
                label: Some("text_texture"),
                size: Extent3d {
                    width: upscale_width,
                    height: upscale_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            &data,
        );

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("text_view"),
            ..Default::default()
        });

        let sampler = render.device.create_sampler(&SamplerDescriptor {
            label: Some("text_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("text_bind"),
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

        let instance = world.insert(TextInstance {
            bind,
            uniform,
            texture,
            sampler,
        });

        let control = world.insert(RenderControl {
            visible: self.visible,
            order: self.order,
        });

        world.observer(control, move |Redraw, world, _| {
            let manager = world.single_fetch::<TextManager>().unwrap();
            let viewport = world.fetch(viewport).unwrap();
            let instance = world.fetch(instance).unwrap();

            let mut render = world.single_fetch_mut::<Render>().unwrap();
            let rpass = &mut render.active.as_mut().unwrap().rpass;
            rpass.set_pipeline(&manager.pipeline);
            rpass.set_bind_group(0, &viewport.bind, &[]);
            rpass.set_bind_group(1, &instance.bind, &[]);
            rpass.draw(0..4, 0..1);
        });

        world.dependency(control, instance);

        Text {
            instance,
            cmd: world.commander(),
        }
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        let instance = self.instance;
        self.cmd.queue(move |world| {
            world.remove(instance);
        });
    }
}
