use glam::{Vec2, Vec4};
use ln_world::{Element, Handle, World};
use lyon::{
    math::{Point, Transform, Vector},
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::{
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{MSAA_STATE, Render, RenderDrawControl, RenderPrepareControl, camera::CameraBind},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, common::WidgetCommon, shaders::shader_compile,
    },
};

pub struct VtorPipeline {
    pipeline: RenderPipeline,
    vtor_bind: BindGroupLayout,
}

pub struct Vtor {
    pub common: WidgetCommon,
    pub rect: Rectangle,
    pub color: Vec4,
    pub order: isize,
    pub path: Path,
    pub outdated: bool,
}

struct VtorInstance {
    len: u32,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    rect_uniform: Buffer,
    color_uniform: Buffer,
    vtor_bind: BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct VtorUniform {
    color: Vec4,
}

pub struct SetVtorColor(pub Vec4);

impl Vtor {
    pub fn init(&mut self, world: &World, this: Handle<Self>) {
        let render = world.single_fetch::<Render>();
        let pipeline = world.single_fetch::<VtorPipeline>();

        self.outdated = false;
        let instance = world.insert(Vtor::gen_path_buffer(
            self.path.clone(),
            self.rect,
            self.color,
            &pipeline,
            &render.device,
        ));

        let prepare = world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let mut this = world.fetch_mut(this);
                let mut instance = world.fetch_mut(instance);
                let render = world.single_fetch::<Render>();
                let pipeline = world.single_fetch::<VtorPipeline>();

                if this.outdated {
                    this.outdated = false;
                    *instance = Vtor::gen_path_buffer(
                        this.path.clone(),
                        this.rect,
                        this.color,
                        &pipeline,
                        &render.device,
                    );
                }

                None
            }),
        });

        let control = world.insert(RenderDrawControl {
            common: self.common,
            order: self.order,
            draw: Box::new(move |world, rpass, extra| {
                let pipeline = world.single_fetch::<VtorPipeline>();
                let instance = world.fetch(instance);
                let camera = extra.camera;

                if instance.len == 0 {
                    return;
                }

                let key = format!("main > vtor");
                let (start, end) = extra.diagnosis.assign_string(key);
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.pipeline);
                rpass.set_vertex_buffer(0, instance.vertex_buffer.slice(..));
                rpass.set_index_buffer(instance.index_buffer.slice(..), IndexFormat::Uint16);
                rpass.set_bind_group(0, &camera.bind, &[]);
                rpass.set_bind_group(1, &instance.vtor_bind, &[]);
                rpass.draw_indexed(0..instance.len, 0, 0..1);

                extra.diagnosis.write(rpass, end);
            }),
        });

        world.observer(this, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(this);
            let instance = world.fetch(instance);
            let render = world.single_fetch::<Render>();
            this.rect = rect;
            let bytes = bytemuck::bytes_of(&rect);
            render.queue.write_buffer(&instance.rect_uniform, 0, bytes);
            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        });

        world.observer(this, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(this);
            this.common.visible = visible;
            world.queue_trigger(control, SetWidgetVisible(visible));
        });

        world.observer(this, move |&SetVtorColor(color), world| {
            let mut this = world.fetch_mut(this);
            let instance = world.fetch(instance);
            let render = world.single_fetch::<Render>();

            this.color = color;
            render
                .queue
                .write_buffer(&instance.color_uniform, 0, bytemuck::bytes_of(&color));
        });

        world.dependency(control, this);
        world.dependency(prepare, this);
    }

    fn gen_path_buffer(
        path: Path,
        rect: Rectangle,
        color: Vec4,
        pipeline: &VtorPipeline,
        device: &Device,
    ) -> VtorInstance {
        let mut tessellator = FillTessellator::new();
        let mut geometry: VertexBuffers<Vec2, u16> = VertexBuffers::new();
        tessellator
            .tessellate_path(
                &path.transformed(&Transform::scale(
                    rect.extend.x as f32,
                    rect.extend.y as f32,
                )),
                &FillOptions::default(),
                &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                    Vec2::from_array(vertex.position().to_array()) / rect.extend.as_vec2()
                }),
            )
            .unwrap();

        let vertices = geometry.vertices;
        let indices = geometry.indices;
        let len = indices.len() as u32;

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
            contents: bytemuck::bytes_of(&rect),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let color_uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("vtor_color"),
            contents: bytemuck::bytes_of(&color),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let vtor_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("vtor_group"),
            layout: &pipeline.vtor_bind,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: rect_uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: color_uniform.as_entire_binding(),
                },
            ],
        });

        VtorInstance {
            len,
            vertex_buffer,
            index_buffer,
            rect_uniform,
            color_uniform,
            vtor_bind,
        }
    }
}

#[expect(unused)]
pub fn logo() -> Path {
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

    builder
        .transformed(Transform::scale(64.0, -64.0).then_translate(Vector::new(0.0, 64.0)))
        .build()
}

impl VtorPipeline {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>();
        let camera = world.single_fetch::<CameraBind>();
        let device = &render.device;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vtor"),
            source: ShaderSource::Wgsl(shader_compile(include_str!("vtor.wgsl"), &[]).into()),
        });

        let vtor_bind = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("vtor"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("vtor"),
            bind_group_layouts: &[Some(&camera.layout), Some(&vtor_bind)],
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
            cache: render.cache.as_ref(),
        });

        VtorPipeline {
            pipeline,
            vtor_bind,
        }
    }
}

impl Element for VtorPipeline {}

impl Element for Vtor {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for VtorInstance {}
