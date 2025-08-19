use std::borrow::Cow;

use bytemuck::bytes_of;
use glam::Vec2;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::dpi::PhysicalSize;

#[derive(Default)]
pub struct Canvas {
    elements: Vec<CanvasInstance>,

    pipeline: Option<RenderPipeline>,
}
impl Canvas {
    pub fn setup(&mut self, device: &Device) {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("vertex_shader.wgsl"))),
        });

        self.pipeline = Some(device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("test_pipeline"),
            layout: None,
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x3,
                        },
                        VertexAttribute {
                            offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                            shader_location: 1,
                            format: VertexFormat::Float32x3,
                        },
                    ],
                }],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(TextureFormat::Bgra8UnormSrgb.into())],
            }),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        }));

        for elem in &mut self.elements {
            elem.setup(device);
        }
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        if let Some(pipeline) = &self.pipeline {
            rpass.set_pipeline(pipeline);
            for elem in &self.elements {
                elem.render(rpass);
            }
        }
    }

    pub fn new_instance(&mut self, left: f32, down: f32, right: f32, up: f32) {
        self.elements
            .push(CanvasInstance::new(left, down, right, up));
    }
}

struct CanvasInstance {
    from: Vec2,
    to: Vec2,

    vertices: Option<Buffer>,
    indices: Option<Buffer>,
}
impl CanvasInstance {
    fn new(left: f32, down: f32, right: f32, up: f32) -> Self {
        CanvasInstance {
            from: Vec2::new(left, down),
            to: Vec2::new(right, up),
            vertices: None,
            indices: None,
        }
    }

    fn setup(&mut self, device: &Device) {
        self.vertices = Some(device.create_buffer_init(&BufferInitDescriptor {
            label: Some("test_buffer"),
            contents: bytes_of(&[
                Vertex {
                    position: [0.0, 0.0, 0.0],
                    color: [1.0, 0.0, 0.0],
                },
                Vertex {
                    position: [0.0, 1.0, 0.0],
                    color: [1.0, 1.0, 0.0],
                },
                Vertex {
                    position: [1.0, 1.0, 0.0],
                    color: [0.0, 1.0, 0.0],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    color: [0.0, 1.0, 1.0],
                },
            ]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        }));
        self.indices = Some(device.create_buffer_init(&BufferInitDescriptor {
            label: Some("test_buffer_indices"),
            contents: bytes_of(&[
                0, 1, 2, 2, 3, 0,
            ]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        }));
    }

    fn render(&self, rpass: &mut RenderPass) {
        let (Some(vertex), Some(indices)) = (&self.vertices, &self.indices) else {
            log::error!("Render is called before setup.");
            return;
        };
        rpass.set_vertex_buffer(0, vertex.slice(..));
        rpass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);
        rpass.draw_indexed(0..6, 0, 0..1);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}
