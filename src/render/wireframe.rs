use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, FragmentState,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind},
        vertex::VertexUniform,
    },
    world::{Descriptor, Element, Handle, World},
};

pub struct Wireframe {
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,
    bind: BindGroup,
    uniform: Buffer,
    queue: Queue,
}

#[derive(Debug, Default)]
pub struct WireframeDescriptor {
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,
}

pub struct WireframeManager {
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct WireframeManagerDescriptor;

impl Descriptor for WireframeManagerDescriptor {
    type Target = Handle<WireframeManager>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();

        let shader = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("wireframe_shader"),
            source: ShaderSource::Wgsl(include_str!("wireframe.wgsl").into()),
        });

        let bind_layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("wireframe_bind_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("wireframe_pipeline_layout"),
                bind_group_layouts: &[&camera.layout, &bind_layout],
                immediate_size: 0,
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("wireframe_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::LineStrip,
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

        world.insert(WireframeManager {
            pipeline,
            bind_layout,
        })
    }
}

impl Element for WireframeManager {}

impl Descriptor for WireframeDescriptor {
    type Target = Handle<Wireframe>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let manager = &mut *world.single_fetch_mut::<WireframeManager>().unwrap();

        // instance //

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wireframe_uniform"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: self.rect.origin.into_array(),
                extend: self.rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("wireframe_bind"),
            layout: &manager.bind_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        world.insert(Wireframe {
            rect: self.rect,
            order: self.order,
            visible: self.visible,
            bind,
            uniform,
            queue: render.queue.clone(),
        })
    }
}

impl Element for Wireframe {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass| {
                let manager = world.single_fetch::<WireframeManager>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();
                let this = world.fetch(this).unwrap();

                rpass.set_pipeline(&manager.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &this.bind, &[]);
                rpass.draw(0..5, 0..1);
            })),
        });

        world.dependency(control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        let uniform = VertexUniform {
            origin: self.rect.origin.into_array(),
            extend: self.rect.extend.into_array(),
        };

        let bytes = bytemuck::bytes_of(&uniform);
        self.queue.write_buffer(&self.uniform, 0, bytes);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}

impl Wireframe {
    pub fn to_descriptor(&self) -> WireframeDescriptor {
        WireframeDescriptor {
            rect: self.rect,
            order: self.order,
            visible: self.visible,
        }
    }
}
