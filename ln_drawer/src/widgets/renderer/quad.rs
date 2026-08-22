use std::marker::PhantomData;

use ln_world::{Descriptor, Element, Handle, World};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::{
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{Camera, CameraBind},
    },
    widgets::shaders::LIB_CAMERA,
};

pub trait QuadMaterial: Clone + Copy + bytemuck::Pod + bytemuck::Zeroable {
    fn label() -> &'static str;

    fn shader() -> ShaderSource<'static>;

    fn vertex() -> Option<Option<&'static str>> {
        None
    }

    fn fragment() -> Option<&'static str>;
}

pub struct QuadMeshPipeline<M: QuadMaterial> {
    pipeline: RenderPipeline,
    bind: BindGroupLayout,
    _marker: PhantomData<M>,
}

// TODO upgrade to modern code style
pub struct QuadMeshDescriptor<M: QuadMaterial> {
    pub rect: Rectangle,
    pub visible: bool,
    pub order: isize,
    pub material: M,
}

pub struct QuadMesh<M: QuadMaterial> {
    pub desc: QuadMeshDescriptor<M>,
    control: Handle<RenderControl>,
    rectangle: Buffer,
    material: Buffer,
    queue: Queue,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleUniform {
    pub origin: [i32; 2],
    pub extend: [u32; 2],
}

impl<M: QuadMaterial> QuadMesh<M> {
    pub fn create(desc: QuadMeshDescriptor<M>, world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<QuadMeshPipeline<M>>().unwrap();
        let device = &render.device;

        let rectangle = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("rectangle"),
            contents: bytemuck::bytes_of(&RectangleUniform {
                origin: desc.rect.origin.into(),
                extend: desc.rect.extend.into(),
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
            draw: Some(Box::new(move |world, rpass, extra| {
                let pipeline = world.single_fetch::<QuadMeshPipeline<M>>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                let key = format!("main > common > {}", M::label());
                let (start, end) = extra.diagnosis.assign_string(key);
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &bind, &[]);
                rpass.draw(0..4, 0..1);

                extra.diagnosis.write(rpass, end);
            })),
        });

        QuadMesh {
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
            origin: self.desc.rect.origin.into(),
            extend: self.desc.rect.extend.into(),
        };

        let rectangle = bytemuck::bytes_of(&rectangle);
        let material = bytemuck::bytes_of(&self.desc.material);

        self.queue.write_buffer(&self.rectangle, 0, rectangle);
        self.queue.write_buffer(&self.material, 0, material);
    }
}

impl<M: QuadMaterial> QuadMeshPipeline<M> {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let quad_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(M::label()),
            source: ShaderSource::Wgsl(
                format!("{}{}", LIB_CAMERA, include_str!("quad.wgsl")).into(),
            ),
        });

        let custom_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(M::label()),
            source: M::shader(),
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
            bind_group_layouts: &[Some(&camera.layout), Some(&bind)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(M::label()),
            layout: Some(&pipeline),
            vertex: match M::vertex() {
                Some(entry_point) => VertexState {
                    module: &custom_shader,
                    entry_point,
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                None => VertexState {
                    module: &quad_shader,
                    entry_point: None,
                    compilation_options: Default::default(),
                    buffers: &[],
                },
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &custom_shader,
                entry_point: M::fragment(),
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

        QuadMeshPipeline {
            pipeline,
            bind,
            _marker: PhantomData::<M>,
        }
    }
}

impl<M: QuadMaterial> Descriptor for QuadMeshDescriptor<M> {
    type Target = Handle<QuadMesh<M>>;
    fn when_build(self, world: &World) -> Self::Target {
        world.insert(QuadMesh::create(self, world))
    }
}

impl<M: QuadMaterial> Element for QuadMeshPipeline<M> {}

impl<M: QuadMaterial> Element for QuadMesh<M> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.reorder(world);
        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        self.reorder(world);
        self.update_buffer();
        RenderControl::redraw(world);
    }
}
