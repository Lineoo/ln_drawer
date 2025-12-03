use std::sync::mpsc::Sender;

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::interface::{Component, ComponentCommand, ComponentInner, Interface};
use crate::{
    measures::{Rectangle, ZOrder},
    tools::pointer::PointerCollider,
    world::{Element, WorldCellEntry},
};

pub struct StandardSquareManager {
    pipeline: RenderPipeline,
    rectangle: BindGroupLayout,
}

pub struct StandardSquareInstance {
    rectangle_bind: BindGroup,
    rectangle_buffer: Buffer,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StandardSquareUniform {
    origin: [i32; 2],
    extend: [i32; 2],
    color: [f32; 4],
}

impl StandardSquareInstance {
    pub fn manager(
        device: &Device,
        viewport: &BindGroupLayout,
        format: TextureFormat,
    ) -> StandardSquareManager {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("standard_square"),
            source: ShaderSource::Wgsl(include_str!("standard_square.wgsl").into()),
        });

        let rectangle = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("standard_square_rectangle"),
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("standard_square"),
            bind_group_layouts: &[viewport, &rectangle],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("standard_square"),
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

        StandardSquareManager {
            pipeline,
            rectangle,
        }
    }

    pub fn create(
        rect: Rectangle,
        color: palette::Srgba,
        device: &Device,
        manager: &StandardSquareManager,
    ) -> StandardSquareInstance {
        let rectangle_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("standard_square_rectangle"),
            contents: bytemuck::bytes_of(&StandardSquareUniform {
                origin: rect.origin.into_array(),
                extend: rect.extend.into_array(),
                color: [color.red, color.blue, color.green, color.alpha],
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let rectangle_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("standard_square_rectangle"),
            layout: &manager.rectangle,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &rectangle_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        StandardSquareInstance {
            rectangle_bind,
            rectangle_buffer,
        }
    }

    pub fn draw(
        &self,
        rpass: &mut RenderPass,
        viewport: &BindGroup,
        manager: &StandardSquareManager,
    ) {
        rpass.set_pipeline(&manager.pipeline);
        rpass.set_bind_group(0, viewport, &[]);
        rpass.set_bind_group(1, &self.rectangle_bind, &[]);
        rpass.draw(0..4, 0..1);
    }
}

pub struct StandardSquare {
    rect: Rectangle,
    z_order: ZOrder,
    visible: bool,

    comp_idx: usize,
    comp_tx: Sender<(usize, ComponentCommand)>,

    queue: Queue,
    rectangle_buffer: Buffer,
}
impl StandardSquare {
    pub fn new(
        rect: Rectangle,
        z_order: ZOrder,
        visible: bool,
        color: palette::Srgba,
        interface: &mut Interface,
    ) -> StandardSquare {
        let instance = StandardSquareInstance::create(
            rect,
            color,
            &interface.device,
            &interface.standard_square,
        );
        let rectangle_buffer = instance.rectangle_buffer.clone();

        interface.insert(Component {
            component: ComponentInner::StandardSquare(instance),
            z_order: z_order.idx,
            visible,
        });

        StandardSquare {
            rect,
            z_order,
            visible,
            rectangle_buffer,
            comp_idx: interface.components_idx - 1,
            comp_tx: interface.components_tx.clone(),
            queue: interface.queue.clone(),
        }
    }

    pub fn get_rect(&self) -> Rectangle {
        self.rect
    }

    pub fn set_rect(&mut self, rect: Rectangle) {
        self.rect = rect;
        self.queue.write_buffer(
            &self.rectangle_buffer,
            0,
            bytemuck::bytes_of(&[rect.origin.into_array(), rect.extend.into_array()]),
        );
    }

    pub fn get_z_order(&self) -> ZOrder {
        self.z_order
    }

    pub fn set_z_order(&mut self, ord: ZOrder) {
        self.z_order = ord;
        let ret =
            (self.comp_tx).send((self.comp_idx, ComponentCommand::SetZOrder(self.z_order.idx)));
        if let Err(e) = ret {
            log::warn!("set z-order: {e}");
        }
    }

    pub fn get_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        let ret =
            (self.comp_tx).send((self.comp_idx, ComponentCommand::SetVisibility(self.visible)));
        if let Err(e) = ret {
            log::warn!("set visibility: {e}");
        }
    }
}

impl Drop for StandardSquare {
    fn drop(&mut self) {
        let ret = (self.comp_tx).send((self.comp_idx, ComponentCommand::Destroy));
        if let Err(e) = ret {
            log::warn!("destroying: {e}");
        }
    }
}

impl Element for StandardSquare {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.getter::<PointerCollider>(|this| PointerCollider {
            rect: this.rect,
            z_order: ZOrder::new(0),
        });

        entry.getter::<Rectangle>(StandardSquare::get_rect);
        entry.setter::<Rectangle>(StandardSquare::set_rect);
    }
}
