use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::measures::Rectangle;
use crate::render::viewport::Viewport;
use crate::render::{Redraw, Render, RenderControl};
use crate::world::{Commander, Descriptor, Element, Handle, World};

pub struct RoundedRect {
    pub rect: Rectangle,
    pub color: palette::Srgba,
    pub order: isize,
    pub visible: bool,

    instance: Handle<RoundedRectInstance>,
    control: Handle<RenderControl>,
    cmd: Commander,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RoundedRectDescriptor {
    pub rect: Rectangle,
    pub color: palette::Srgba,
    pub order: isize,
    pub visible: bool,
}

pub struct RoundedRectManager {
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct RoundedRectManagerDescriptor;

pub struct RoundedRectInstance {
    bind: BindGroup,
    uniform: Buffer,
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

impl Element for RoundedRect {}
impl Element for RoundedRectManager {}
impl Element for RoundedRectInstance {}

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

impl Descriptor for RoundedRectDescriptor {
    type Target = RoundedRect;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single::<Viewport>().unwrap();
        let manager = world.single_fetch::<RoundedRectManager>().unwrap();

        // instance //

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
                shrink: 5.0,
                value: 5.0,
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

        let instance = world.insert(RoundedRectInstance { bind, uniform });

        let control = world.insert(RenderControl {
            visible: self.visible,
            order: self.order,
        });

        world.observer(control, move |Redraw, world, _| {
            let manager = world.single_fetch::<RoundedRectManager>().unwrap();
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

        RoundedRect {
            rect: self.rect,
            color: self.color,
            order: self.order,
            visible: self.visible,
            instance,
            control,
            cmd: world.commander(),
        }
    }
}

impl RoundedRect {
    pub fn upload(&self) {
        let instance = self.instance;
        let control = self.control;
        let visible = self.visible;
        let order = self.order;
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
            shrink: 5.0,
            value: 5.0,
            _pad: 0,
        };

        self.cmd.queue(move |world| {
            let instance = world.fetch(instance).unwrap();
            let render = world.single_fetch::<Render>().unwrap();
            let bytes = bytemuck::bytes_of(&uniform);
            render.queue.write_buffer(&instance.uniform, 0, bytes);

            let mut control = world.fetch_mut(control).unwrap();
            control.order = order;
            control.visible = visible;
        });
    }
}

impl Drop for RoundedRect {
    fn drop(&mut self) {
        let instance = self.instance;
        self.cmd.queue(move |world| {
            world.remove(instance);
        });
    }
}
