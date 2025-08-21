use std::sync::mpsc::{Receiver, Sender, channel};

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

pub struct WireframePipeline {
    pipeline: RenderPipeline,
    removal_tx: Sender<usize>,
    removal_rx: Receiver<usize>,
    wireframe: Vec<WireframeBuffer>,
}

impl WireframePipeline {
    pub fn init(device: &Device, surface: &SurfaceConfiguration) -> WireframePipeline {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("wireframe_shader"),
            source: ShaderSource::Wgsl(include_str!("wireframe.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("wireframe_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("wireframe_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("wireframe_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x2,
                    }],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(surface.format.into())],
            }),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let (removal_tx, removal_rx) = channel();

        WireframePipeline {
            pipeline,
            removal_tx,
            removal_rx,
            wireframe: Vec::new(),
        }
    }

    #[must_use = "The wireframe will be destroyed when being drop."]
    pub fn create(
        &mut self,
        rect: [f32; 4],
        color: [f32; 4],
        device: &Device,
        queue: &Queue,
    ) -> Wireframe {
        self.clean();

        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wireframe_vertex_buffer"),
            contents: bytemuck::bytes_of(&[
                [rect[0], rect[1]],
                [rect[0], rect[3]],
                [rect[2], rect[3]],
                [rect[2], rect[1]],
            ]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wireframe_index_buffer"),
            contents: bytemuck::bytes_of(&[0, 1, 1, 2, 2, 3, 3, 0]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        let color = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("color_uniform"),
            contents: bytemuck::bytes_of(&color),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("wireframe_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("wireframe_bind_group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &color,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let wireframe = WireframeBuffer {
            vertices,
            indices,
            bind_group,
            color,
        };

        self.wireframe.push(wireframe.clone());

        Wireframe {
            rect,
            pipeline_remove: self.removal_tx.clone(),
            pipeline_remove_idx: self.wireframe.len() - 1,
            buffer: wireframe,
            queue: queue.clone(),
        }
    }

    pub fn clean(&mut self) {
        for idx in self.removal_rx.try_iter() {
            self.wireframe.swap_remove(idx);
        }
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        for wireframe in &self.wireframe {
            rpass.set_bind_group(0, Some(&wireframe.bind_group), &[]);
            rpass.set_vertex_buffer(0, wireframe.vertices.slice(..));
            rpass.set_index_buffer(wireframe.indices.slice(..), IndexFormat::Uint32);
            rpass.draw_indexed(0..8, 0, 0..1);
        }
    }
}

pub struct Wireframe {
    /// rect: [left, down, right, up]
    rect: [f32; 4],

    pipeline_remove_idx: usize,
    pipeline_remove: Sender<usize>,
    buffer: WireframeBuffer,
    queue: Queue,
}
impl Drop for Wireframe {
    fn drop(&mut self) {
        if let Err(e) = self.pipeline_remove.send(self.pipeline_remove_idx) {
            log::warn!("Dropping Wireframe: {e}");
        }
    }
}
impl Wireframe {
    pub fn set_rect(&self, rect: [f32; 4]) {
        let contents = [
            [rect[0], rect[1]],
            [rect[0], rect[3]],
            [rect[2], rect[3]],
            [rect[2], rect[1]],
        ];
        self.queue
            .write_buffer(&self.buffer.vertices, 0, bytemuck::bytes_of(&contents));
    }
    pub fn set_color(&self, color: [f32; 4]) {
        self.queue
            .write_buffer(&self.buffer.color, 0, bytemuck::bytes_of(&color));
    }
}

#[derive(Clone)]
pub struct WireframeBuffer {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    color: Buffer,
}