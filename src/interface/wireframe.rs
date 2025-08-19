use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

#[derive(Clone)]
pub struct Wireframe {
    vertices: Buffer,
    indices: Buffer,

    bind_group: BindGroup,
    color: Buffer,
}
impl Wireframe {
    /// rect: [left, down, right, up]
    pub fn init(rect: [f32; 4], color: [f32; 4], device: &Device) -> Wireframe {
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
            usage: BufferUsages::UNIFORM,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("color_bind_group_layout"),
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
            label: Some("color_bind_group"),
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

        Wireframe {
            vertices,
            indices,
            bind_group,
            color,
        }
    }
    pub fn set_rect(&self, rect: [f32; 4], queue: &Queue) {
        let contents = [
            [rect[0], rect[1]],
            [rect[0], rect[3]],
            [rect[2], rect[3]],
            [rect[2], rect[1]],
        ];
        queue.write_buffer(&self.vertices, 0, bytemuck::bytes_of(&contents));
    }
    pub fn set_color(&self, color: [f32; 4], queue: &Queue) {
        queue.write_buffer(&self.color, 0, bytemuck::bytes_of(&color));
    }
    fn render(&self, rpass: &mut RenderPass) {
        rpass.set_bind_group(0, Some(&self.bind_group), &[]);
        rpass.set_vertex_buffer(0, self.vertices.slice(..));
        rpass.set_index_buffer(self.indices.slice(..), IndexFormat::Uint32);
        rpass.draw_indexed(0..8, 0, 0..1);
    }
}

pub struct WireframePipeline {
    pipeline: RenderPipeline,
    wireframe: Vec<Wireframe>,
}
impl WireframePipeline {
    pub fn init(device: &Device) -> WireframePipeline {
        // TODO: this shader will be provided by interface instead then
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("color_shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("color_bind_group_layout"),
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
            label: Some("color_layout"),
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
                targets: &[Some(TextureFormat::Bgra8UnormSrgb.into())],
            }),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        WireframePipeline {
            pipeline,
            wireframe: Vec::new(),
        }
    }

    pub fn create(&mut self, rect: [f32; 4], color: [f32; 4], device: &Device) -> Wireframe {
        let wireframe = Wireframe::init(rect, color, device);
        self.wireframe.push(wireframe.clone());
        wireframe
    }

    pub fn render(&self, rpass: &mut RenderPass) {
        rpass.set_pipeline(&self.pipeline);
        for i in &self.wireframe {
            i.render(rpass);
        }
    }
}
