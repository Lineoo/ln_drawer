use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::measures::Rectangle;
use crate::render::viewport::Viewport;
use crate::render::{Redraw, Render, RenderControl};
use crate::world::{Commander, Descriptor, Element, Handle, World};

pub struct RoundedRectManagerDescriptor;

#[derive(Debug, Clone, Copy)]
pub struct RoundedRectDescriptor {
    pub rect: Rectangle,
    pub color: palette::Srgba,
    pub shrink: f32,
    pub value: f32,
    pub visible: bool,
    pub order: isize,
}

pub struct RoundedRectManager {
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct RoundedRect {
    pub rect: Rectangle,
    pub color: palette::Srgba,
    pub shrink: f32,
    pub value: f32,
    pub visible: bool,
    pub order: isize,

    bind: BindGroup,
    uniform: Buffer,
    queue: Queue,

    control: Handle<RenderControl>,
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

impl Descriptor for RoundedRectManagerDescriptor {
    type Target = Handle<RoundedRectManager>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single_fetch::<Viewport>().unwrap();

        let caps = render.surface.get_capabilities(&render.adapter);
        let format = *caps.formats.first().unwrap();

        let shader = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("rounded_shader"),
            source: ShaderSource::Wgsl(include_str!("rounded.wgsl").into()),
        });

        let bind_layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("rounded_bind_layout"),
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
                label: Some("rounded_pipeline_layout"),
                bind_group_layouts: &[&viewport.layout, &bind_layout],
                push_constant_ranges: &[],
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("rounded_pipeline"),
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

        world.insert(RoundedRectManager {
            pipeline,
            bind_layout,
        })
    }
}

impl Element for RoundedRectManager {}

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

impl Descriptor for RoundedRectDescriptor {
    type Target = Handle<RoundedRect>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let manager = world.single_fetch::<RoundedRectManager>().unwrap();

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("rounded_buffer"),
            contents: bytemuck::bytes_of(&RoundedRectUniform {
                origin: self.rect.origin.into_array(),
                extend: self.rect.extend.into_array(),
                color: [
                    self.color.red,
                    self.color.green,
                    self.color.blue,
                    self.color.alpha,
                ],
                vertex_extend: 10,
                shrink: self.shrink,
                value: self.value,
                _pad: 0,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("rounded_bind_group"),
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

        let control = world.insert(RenderControl {
            visible: self.visible,
            order: self.order,
        });

        world.insert(RoundedRect {
            rect: self.rect,
            color: self.color,
            shrink: self.shrink,
            value: self.value,
            order: self.order,
            visible: self.visible,
            bind,
            uniform,
            queue: render.queue.clone(),
            control,
        })
    }
}

impl Element for RoundedRect {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.control, move |Redraw, world, _| {
            let manager = world.single_fetch::<RoundedRectManager>().unwrap();
            let viewport = world.single_fetch::<Viewport>().unwrap();
            let this = world.fetch(this).unwrap();

            let mut render = world.single_fetch_mut::<Render>().unwrap();
            let rpass = &mut render.active.as_mut().unwrap().rpass;
            rpass.set_pipeline(&manager.pipeline);
            rpass.set_bind_group(0, &viewport.bind, &[]);
            rpass.set_bind_group(1, &this.bind, &[]);
            rpass.draw(0..4, 0..1);
        });

        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        let uniform = RoundedRectUniform {
            origin: self.rect.origin.into_array(),
            extend: self.rect.extend.into_array(),
            color: [
                self.color.red,
                self.color.green,
                self.color.blue,
                self.color.alpha,
            ],
            vertex_extend: 10,
            shrink: self.shrink,
            value: self.value,
            _pad: 0,
        };

        let bytes = bytemuck::bytes_of(&uniform);
        self.queue.write_buffer(&self.uniform, 0, bytes);

        let control = self.control;
        let order = self.order;
        let visible = self.visible;

        let mut control = world.fetch_mut(control).unwrap();
        control.order = order;
        control.visible = visible;
    }
}
