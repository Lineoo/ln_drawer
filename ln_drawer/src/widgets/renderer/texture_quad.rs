use glam::UVec2;
use ln_world::{Element, Handle, World};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    Buffer, BufferBindingType, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Extent3d, FilterMode, FragmentState, LoadOp, Operations,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    layer::{Layer, LayerPipeline},
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{Camera, CameraBind, CurrentCamera},
    },
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, renderer::canvas::RectangleUniform,
        shaders::shader_compile,
    },
};

/// A widget that displays an engine render target.
///
/// The texture is `Rgba8Unorm` so it can be used directly as the color target of
/// [`LayerPipeline::render`], which is compiled for that format. Rendering into it is driven by
/// [`TextureQuad::render_layer`]; this widget only owns the target and its display quad.
pub struct TextureQuad {
    pub rect: Rectangle,
    pub visible: bool,
    pub order: isize,
    pub size: UVec2,

    pub view: TextureView,

    rectangle_uniform: Buffer,
    bind: BindGroup,
}

pub struct TextureQuadPipeline {
    pipeline: RenderPipeline,
    instance: BindGroupLayout,
}

impl TextureQuad {
    pub fn new(world: &World, size: UVec2, order: isize) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<TextureQuadPipeline>().unwrap();
        let device = &render.device;

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("texture_quad_texture"),
            size: Extent3d {
                width: size.x.max(1),
                height: size.y.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("texture_quad_texture_view"),
            ..Default::default()
        });

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
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        TextureQuad {
            rect: Rectangle::default(),
            visible: false,
            order,
            size,
            view,
            rectangle_uniform,
            bind,
        }
    }

    pub fn init(&self, world: &World, this: Handle<Self>) {
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
                rpass.set_bind_group(1, &quad.bind, &[]);
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
                &this.rectangle_uniform,
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

    /// Render `layer` into this quad's texture with `camera`.
    pub fn render_layer(
        &self,
        render: &Render,
        layer_pipeline: &LayerPipeline,
        layer: &Layer,
        camera: &Camera,
        debug: bool,
    ) {
        let mut encoder = render
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("texture_quad_render"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("texture_quad_render"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            layer_pipeline.render(layer, &mut rpass, camera, debug, false);
        }

        render.queue.submit([encoder.finish()]);
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
