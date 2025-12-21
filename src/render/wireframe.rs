use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, FragmentState,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::{
    measures::Rectangle,
    render::{
        Redraw, Render, RenderActive, RenderControl,
        viewport::{ViewportInstance, ViewportManager},
    },
    world::{Commander, Descriptor, Element, Handle, World},
};

pub struct Wireframe {
    pub rect: Rectangle,
    pub order: isize,
    pub visible: bool,
    instance: Handle<WireframeInstance>,
    control: Handle<RenderControl>,
    cmd: Commander,
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

pub struct WireframeInstance {
    bind: BindGroup,
    uniform: Buffer,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WireframeUniform {
    origin: [i32; 2],
    extend: [i32; 2],
}

impl Element for Wireframe {}
impl Element for WireframeManager {}
impl Element for WireframeInstance {}

impl Descriptor for WireframeManagerDescriptor {
    type Target = WireframeManager;

    fn build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single_fetch::<ViewportManager>().unwrap();

        let caps = render.surface.get_capabilities(&render.adapter);
        let format = *caps.formats.first().unwrap();

        let shader_vs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: ShaderSource::Wgsl(include_str!("vertex.wgsl").into()),
        });

        let shader_fs = render.device.create_shader_module(ShaderModuleDescriptor {
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
                bind_group_layouts: &[&viewport.layout, &bind_layout],
                push_constant_ranges: &[],
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("wireframe_pipeline"),
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

        WireframeManager {
            pipeline,
            bind_layout,
        }
    }
}

impl Descriptor for WireframeDescriptor {
    type Target = Wireframe;

    fn build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single::<ViewportInstance>().unwrap();
        let manager = &mut *world.single_fetch_mut::<WireframeManager>().unwrap();

        // instance //

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wireframe_uniform"),
            contents: bytemuck::bytes_of(&WireframeUniform {
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

        let instance = world.insert(WireframeInstance { bind, uniform });

        let control = world.insert(RenderControl {
            visible: self.visible,
            order: self.order,
        });

        world.observer(control, move |Redraw, world, _| {
            let manager = world.single_fetch::<WireframeManager>().unwrap();
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

        Wireframe {
            rect: self.rect,
            order: self.order,
            visible: self.visible,
            instance,
            control,
            cmd: world.commander(),
        }
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

    /// upload all public fields to GPU and corresponding control
    pub fn upload(&self) {
        let instance = self.instance;
        let control = self.control;
        let visible = self.visible;
        let order = self.order;
        let uniform = WireframeUniform {
            origin: self.rect.origin.into_array(),
            extend: self.rect.extend.into_array(),
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

impl Drop for Wireframe {
    fn drop(&mut self) {
        let instance = self.instance;
        self.cmd.queue(move |world| {
            world.remove(instance);
        });
    }
}
