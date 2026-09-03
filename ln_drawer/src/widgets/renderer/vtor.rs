use glam::Vec2;
use ln_world::{Element, Handle, World};
use lyon::{
    math::Point,
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::{
    measures::Rectangle,
    render::{
        MSAA_STATE, Render, RenderControl,
        camera::{CameraBind, CurrentCamera},
    },
    widgets::{SetWidgetRectangle, SetWidgetVisible, shaders::shader_compile},
};

pub struct VtorPipeline {
    pipeline: RenderPipeline,
    bind: BindGroupLayout,
}

pub struct Vtor {
    pub rect: Rectangle,
    pub visible: bool,
    pub order: isize,
}

impl Vtor {
    pub fn init(&self, world: &World, this: Handle<Self>) {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<VtorPipeline>().unwrap();
        let device = &render.device;

        let mut builder = Path::svg_builder();

        // ========== Path 1 ==========
        builder.move_to(Point::new(13.0, 43.0));
        builder.line_to(Point::new(11.0, 48.0));
        builder.line_to(Point::new(37.0, 48.0));
        builder.line_to(Point::new(39.0, 43.0));
        builder.line_to(Point::new(22.0, 43.0));
        builder.line_to(Point::new(22.0, 16.0));
        builder.line_to(Point::new(15.0, 14.0));
        builder.line_to(Point::new(15.0, 43.0));
        builder.close();

        // ========== Path 2 ==========
        builder.move_to(Point::new(28.0, 37.0));
        builder.line_to(Point::new(35.0, 39.0));
        builder.line_to(Point::new(35.0, 27.0));
        builder.line_to(Point::new(39.0, 27.0));
        builder.cubic_bezier_to(
            Point::new(41.519, 27.0),
            Point::new(43.0, 27.0),
            Point::new(43.0, 33.0),
        );
        builder.line_to(Point::new(43.0, 48.0));
        builder.line_to(Point::new(50.0, 48.0));
        builder.line_to(Point::new(50.0, 33.0));
        builder.cubic_bezier_to(
            Point::new(50.0, 23.0),
            Point::new(48.444, 23.0),
            Point::new(39.0, 23.0),
        );
        builder.line_to(Point::new(28.0, 23.0));
        builder.close();

        let path = builder.build();

        let mut tessellator = FillTessellator::new();
        let mut geometry: VertexBuffers<Vec2, u16> = VertexBuffers::new();
        tessellator
            .tessellate_path(
                &path,
                &FillOptions::default(),
                &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                    Vec2::from_array(vertex.position().to_array()) * Vec2::new(1., -1.)
                }),
            )
            .unwrap();

        let vertices = dbg!(geometry.vertices);
        let indices = dbg!(geometry.indices);
        let l = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("vtor_vertices"),
            contents: bytemuck::cast_slice(&vertices[..]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("vtor_indices"),
            contents: bytemuck::cast_slice(&indices[..]),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });

        let rect_uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("vtor_rect"),
            contents: bytemuck::bytes_of(&self.rect),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let rect_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("vtor_group"),
            layout: &pipeline.bind,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: rect_uniform.as_entire_binding(),
            }],
        });

        let control = world.insert(RenderControl {
            prepare: None,
            draw: Some(Box::new(move |world, rpass, extra| {
                let pipeline = world.single_fetch::<VtorPipeline>().unwrap();
                let current_camera = world.single_fetch::<CurrentCamera>().unwrap();
                let camera = world.fetch(current_camera.0).unwrap();

                let key = format!("main > vtor");
                let (start, end) = extra.diagnosis.assign_string(key);
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                rpass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &rect_bind, &[]);
                rpass.draw_indexed(0..l, 0, 0..1);

                extra.diagnosis.write(rpass, end);
            })),
        });

        RenderControl::reorder(self.visible.then_some(self.order), world, control);

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this).unwrap();
            let render = world.single_fetch::<Render>().unwrap();
            this.rect = rect;
            let bytes = bytemuck::bytes_of(&rect);
            render.queue.write_buffer(&rect_uniform, 0, bytes);
            RenderControl::request_redraw(world);
        });

        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.visible = visible;
            RenderControl::reorder(visible.then_some(this.order), world, control);
            RenderControl::request_redraw(world);
        });
    }
}

impl VtorPipeline {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vtor"),
            source: ShaderSource::Wgsl(shader_compile(include_str!("vtor.wgsl"), &[]).into()),
        });

        let rect_bind = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("vtor"),
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
            label: Some("vtor"),
            bind_group_layouts: &[Some(&camera.layout), Some(&rect_bind)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("vtor"),
            layout: Some(&pipeline),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(VertexBufferLayout {
                    array_stride: size_of::<Vec2>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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

        VtorPipeline {
            pipeline,
            bind: rect_bind,
        }
    }
}

impl Element for VtorPipeline {}

impl Element for Vtor {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
