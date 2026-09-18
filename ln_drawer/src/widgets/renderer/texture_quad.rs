use ln_world::{Element, Handle, World};
use wgpu::{
    AddressMode, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferBindingType,
    BufferUsages, ColorTargetState, ColorWrites, FilterMode, FragmentState,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, TextureSampleType, TextureView, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{CameraBind, CurrentCamera},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, renderer::canvas::RectangleUniform,
        shaders::shader_compile,
    },
};

/// Displays an externally owned [`Texture`] inside a widget rectangle.
///
/// The texture is sampled as sRGB-encoded, premultiplied data (matching
/// [`crate::layer::CHUNK_TEXTURE_FORMAT`]), so it can show a layer chunk directly.
pub struct TextureQuad {
    pub rect: Rectangle,
    pub visible: bool,
    pub order: isize,
    pub view: TextureView,
}

pub struct TextureQuadPipeline {
    pipeline: RenderPipeline,
    instance: BindGroupLayout,
}

impl TextureQuad {
    pub fn init(&self, world: &World, this: Handle<Self>) {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<TextureQuadPipeline>().unwrap();
        let device = &render.device;

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("texture_quad_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let rectangle_uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("texture_quad_uniform"),
            contents: bytemuck::bytes_of(&RectangleUniform {
                origin: [0, 0],
                extend: [0, 0],
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("texture_quad"),
            layout: &pipeline.instance,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: rectangle_uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass, extra| {
                let quad = world.fetch(this).unwrap();
                if !quad.visible {
                    return;
                }

                let pipeline = world.single_fetch::<TextureQuadPipeline>().unwrap();
                let current_camera = world.single_fetch::<CurrentCamera>().unwrap();
                let camera = world.fetch(current_camera.0).unwrap();

                let (start, end) = extra.diagnosis.assign("main > texture_quad");
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &bind, &[]);
                rpass.draw(0..4, 0..1);

                extra.diagnosis.write(rpass, end);
            })),
        });

        RenderControl::reorder(self.visible.then_some(self.order), world, control);

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.rect = rect;

            let render = world.single_fetch::<Render>().unwrap();
            render.queue.write_buffer(
                &rectangle_uniform,
                0,
                bytemuck::bytes_of(&RectangleUniform {
                    origin: rect.origin.into(),
                    extend: rect.extend.into(),
                }),
            );
            RenderControl::request_redraw(world);
        });

        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.visible = visible;
            RenderControl::reorder(visible.then_some(this.order), world, control);
            RenderControl::request_redraw(world);
        });
    }
}

impl TextureQuadPipeline {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();

        let shader = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("texture_quad_shader"),
            source: ShaderSource::Wgsl(
                shader_compile(include_str!("texture_quad.wgsl"), &[]).into(),
            ),
        });

        let instance = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("texture_quad_bind_layout"),
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
                ],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("texture_quad_pipeline_layout"),
                bind_group_layouts: &[Some(&camera.layout), Some(&instance)],
                immediate_size: 0,
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("texture_quad_pipeline"),
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
                        blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                depth_stencil: None,
                multisample: MSAA_STATE,
                multiview_mask: None,
                cache: None,
            });

        TextureQuadPipeline { pipeline, instance }
    }
}

impl Element for TextureQuad {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for TextureQuadPipeline {}
