use std::sync::mpsc::{Receiver, Sender, channel};

use hashbrown::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::interface::InterfaceViewport;

pub struct WireframePipeline {
    pipeline: RenderPipeline,

    removal_tx: Sender<usize>,
    removal_rx: Receiver<usize>,

    rect_tx: Sender<(usize, [i32; 4])>,
    rect_rx: Receiver<(usize, [i32; 4])>,

    wireframe_idx: usize,
    wireframe: HashMap<usize, WireframeBuffer>,
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
        let (rect_tx, rect_rx) = channel();

        WireframePipeline {
            pipeline,
            removal_tx,
            removal_rx,
            rect_tx,
            rect_rx,
            wireframe_idx: 0,
            wireframe: HashMap::new(),
        }
    }

    #[must_use = "The wireframe will be destroyed when being drop."]
    pub fn create(
        &mut self,
        rect: [i32; 4],
        color: [f32; 4],
        device: &Device,
        queue: &Queue,
        viewport: &InterfaceViewport,
    ) -> Wireframe {
        self.clean();

        let point_from = viewport.world_to_screen([rect[0], rect[1]]);
        let point_to = viewport.world_to_screen([rect[2], rect[3]]);

        let rect_screen = [point_from[0], point_from[1], point_to[0], point_to[1]];

        let vertices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("wireframe_vertex_buffer"),
            contents: bytemuck::bytes_of(&[
                [rect_screen[0], rect_screen[1]],
                [rect_screen[0], rect_screen[3]],
                [rect_screen[2], rect_screen[3]],
                [rect_screen[2], rect_screen[1]],
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

        self.wireframe.insert(self.wireframe_idx, wireframe.clone());
        self.wireframe_idx += 1;

        Wireframe {
            rect,
            pipeline_remove: self.removal_tx.clone(),
            pipeline_idx: self.wireframe.len() - 1,
            pipeline_rect: self.rect_tx.clone(),
            buffer: wireframe,
            queue: queue.clone(),
        }
    }

    pub fn update_rect(&self, viewport: &InterfaceViewport, queue: &Queue) {
        for (idx, rect) in self.rect_rx.try_iter() {
            let point_from = viewport.world_to_screen([rect[0], rect[1]]);
            let point_to = viewport.world_to_screen([rect[2], rect[3]]);

            let rect_screen = [point_from[0], point_from[1], point_to[0], point_to[1]];

            queue.write_buffer(
                &self.wireframe[&idx].vertices,
                0,
                bytemuck::bytes_of(&[
                    [rect_screen[0], rect_screen[1]],
                    [rect_screen[0], rect_screen[3]],
                    [rect_screen[2], rect_screen[3]],
                    [rect_screen[2], rect_screen[1]],
                ]),
            );
        }
    }

    pub fn clean(&mut self) {
        for idx in self.removal_rx.try_iter() {
            self.wireframe.remove(&idx);
        }
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        for wireframe in self.wireframe.values() {
            rpass.set_bind_group(0, Some(&wireframe.bind_group), &[]);
            rpass.set_vertex_buffer(0, wireframe.vertices.slice(..));
            rpass.set_index_buffer(wireframe.indices.slice(..), IndexFormat::Uint32);
            rpass.draw_indexed(0..8, 0, 0..1);
        }
    }
}

pub struct Wireframe {
    /// rect: [left, down, right, up]
    rect: [i32; 4],

    pipeline_idx: usize,
    pipeline_remove: Sender<usize>,
    pipeline_rect: Sender<(usize, [i32; 4])>,

    buffer: WireframeBuffer,
    queue: Queue,
}
impl Drop for Wireframe {
    fn drop(&mut self) {
        if let Err(e) = self.pipeline_remove.send(self.pipeline_idx) {
            log::warn!("Dropping Wireframe: {e}");
        }
    }
}
impl Wireframe {
    pub fn set_rect(&mut self, rect: [i32; 4]) {
        self.rect = rect;
        if let Err(e) = self.pipeline_rect.send((self.pipeline_idx, rect)) {
            log::error!("Send rect: {e}");
        }
    }
    pub fn set_color(&mut self, color: [f32; 4]) {
        self.queue
            .write_buffer(&self.buffer.color, 0, bytemuck::bytes_of(&color));
    }
}

#[derive(Clone)]
struct WireframeBuffer {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    color: Buffer,
}
