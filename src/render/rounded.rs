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

#[derive(Debug, Clone, Copy)]
pub struct RoundedRectDescriptor {
    pub rect: Rectangle,
    pub color: palette::Srgba,
    pub shrink: f32,
    pub value: f32,
    pub visible: bool,
    pub order: isize,
}

pub struct RoundedRectPipeline {
    pipeline: RenderPipeline,
    bind: BindGroupLayout,
}

pub struct RoundedRect {
    pub desc: RoundedRectDescriptor,
    control: Handle<RenderControl>,
    uniform: Buffer,
    queue: Queue,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RoundedRectUniform {
    origin: [i32; 2],
    extend: [u32; 2],
    color: [f32; 4],
    vertex_extend: i32,
    shrink: f32,
    value: f32,

    _pad: i32,
}

impl Default for RoundedRectDescriptor {
    fn default() -> Self {
        Self {
            rect: Rectangle::new(0, 0, 100, 100),
            color: palette::Srgba::new(1.0, 1.0, 1.0, 1.0),
            shrink: 5.0,
            value: 5.0,
            order: 0,
            visible: true,
        }
    }
}

impl RoundedRect {
    pub fn init(world: &World) {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let shader = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("rounded"),
            source: ShaderSource::Wgsl(include_str!("rounded.wgsl").into()),
        });

        let bind = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("rounded"),
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

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("rounded"),
            bind_group_layouts: &[&camera.layout, &bind],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("rounded"),
            layout: Some(&pipeline),
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

        world.insert(RoundedRectPipeline { pipeline, bind });
    }

    pub fn create(desc: RoundedRectDescriptor, world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let manager = world.single_fetch::<RoundedRectPipeline>().unwrap();

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("rounded"),
            contents: bytemuck::bytes_of(&RoundedRectUniform {
                origin: desc.rect.origin.into_array(),
                extend: desc.rect.extend.into_array(),
                color: [
                    desc.color.red,
                    desc.color.green,
                    desc.color.blue,
                    desc.color.alpha,
                ],
                vertex_extend: 10,
                shrink: desc.shrink,
                value: desc.value,
                _pad: 0,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("rounded"),
            layout: &manager.bind,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass| {
                let manager = world.single_fetch::<RoundedRectPipeline>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                rpass.set_pipeline(&manager.pipeline);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &bind, &[]);
                rpass.draw(0..4, 0..1);
            })),
        });

        RoundedRect {
            desc,
            control,
            uniform,
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
        let uniform = RoundedRectUniform {
            origin: self.desc.rect.origin.into_array(),
            extend: self.desc.rect.extend.into_array(),
            color: [
                self.desc.color.red,
                self.desc.color.green,
                self.desc.color.blue,
                self.desc.color.alpha,
            ],
            vertex_extend: 10,
            shrink: self.desc.shrink,
            value: self.desc.value,
            _pad: 0,
        };

        let bytes = bytemuck::bytes_of(&uniform);
        self.queue.write_buffer(&self.uniform, 0, bytes);
    }
}

impl Descriptor for RoundedRectDescriptor {
    type Target = Handle<RoundedRect>;

    fn when_build(self, world: &World) -> Self::Target {
        world.insert(RoundedRect::create(self, world))
    }
}

impl Element for RoundedRectPipeline {}

impl Element for RoundedRect {
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
