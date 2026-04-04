use std::marker::PhantomData;

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::{
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{Camera, CameraBind},
    },
    world::{Descriptor, Element, Handle, World},
};

pub trait RectangleMeshMaterial: Clone + Copy + bytemuck::Pod + bytemuck::Zeroable {
    fn label() -> &'static str;
    fn fragment() -> ShaderSource<'static>;
    fn entry_point() -> Option<&'static str>;
}

pub struct RectangleMeshPipeline<M: RectangleMeshMaterial> {
    pipeline: RenderPipeline,
    bind: BindGroupLayout,
    _marker: PhantomData<M>,
}

pub struct RectangleMeshDescriptor<M: RectangleMeshMaterial> {
    pub rect: Rectangle,
    pub visible: bool,
    pub order: isize,
    pub material: M,
}

pub struct RectangleMesh<M: RectangleMeshMaterial> {
    pub desc: RectangleMeshDescriptor<M>,
    control: Handle<RenderControl>,
    rectangle: Buffer,
    material: Buffer,
    queue: Queue,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectangleUniform {
    origin: [i32; 2],
    extend: [u32; 2],
}

impl<M: RectangleMeshMaterial> RectangleMesh<M> {
    pub fn init(world: &World) {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let vertex = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(M::label()),
            source: ShaderSource::Wgsl(include_str!("rectangle.wgsl").into()),
        });

        let fragment = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(M::label()),
            source: M::fragment(),
        });

        let bind = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(M::label()),
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
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(M::label()),
            bind_group_layouts: &[&camera.layout, &bind],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(M::label()),
            layout: Some(&pipeline),
            vertex: VertexState {
                module: &vertex,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &fragment,
                entry_point: M::entry_point(),
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

        world.insert(RectangleMeshPipeline {
            pipeline,
            bind,
            _marker: PhantomData::<M>,
        });
    }

    pub fn create(desc: RectangleMeshDescriptor<M>, world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<RectangleMeshPipeline<M>>().unwrap();
        let device = &render.device;

        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("rectangle"),
            contents: bytemuck::bytes_of(&RectangleUniform {
                origin: desc.rect.origin.into_array(),
                extend: desc.rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let material = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(M::label()),
            contents: bytemuck::bytes_of(&desc.material),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some(M::label()),
            layout: &pipeline.bind,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: rectangle.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: material.as_entire_binding(),
                },
            ],
        });

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass| {
                let pipeline = world.single_fetch::<RectangleMeshPipeline<M>>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &bind, &[]);
                rpass.draw(0..4, 0..1);
            })),
        });

        RectangleMesh {
            desc,
            control,
            rectangle,
            material,
            queue: render.queue.clone(),
        }
    }

    fn reorder(&mut self, world: &World) {
        RenderControl::reorder(
            self.desc.visible.then_some(self.desc.order),
            world,
            self.control,
        );
    }

    fn update_buffer(&mut self) {
        let rectangle = RectangleUniform {
            origin: self.desc.rect.origin.into_array(),
            extend: self.desc.rect.extend.into_array(),
        };

        let rectangle = bytemuck::bytes_of(&rectangle);
        let material = bytemuck::bytes_of(&self.desc.material);

        self.queue.write_buffer(&self.rectangle, 0, rectangle);
        self.queue.write_buffer(&self.material, 0, material);
    }
}

impl<M: RectangleMeshMaterial> Descriptor for RectangleMeshDescriptor<M> {
    type Target = Handle<RectangleMesh<M>>;
    fn when_build(self, world: &World) -> Self::Target {
        world.insert(RectangleMesh::create(self, world))
    }
}

impl<M: RectangleMeshMaterial> Element for RectangleMeshPipeline<M> {}

impl<M: RectangleMeshMaterial> Element for RectangleMesh<M> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.reorder(world);
        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        self.reorder(world);
        self.update_buffer();
        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}
