use cosmic_text::{Attrs, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, Extent3d, FilterMode,
    FragmentState, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDescriptor,
    TextureViewDimension, VertexState,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::TextureDataOrder,
};

use crate::{
    measures::Rectangle,
    render::{
        Redraw, Render, RenderControl,
        vertex::VertexUniform,
        viewport::{ViewportInstance, ViewportManager},
    },
    world::{Commander, Descriptor, Element, Handle, World},
};

pub struct Text {
    instance: Handle<TextInstance>,
    cmd: Commander,
}

#[derive(Debug)]
pub struct TextDescriptor<'a> {
    pub text: &'a str,
    pub rect: Rectangle,
    pub metrics: Metrics,
    pub order: isize,
    pub visible: bool,
}

pub struct TextManager {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pipeline: RenderPipeline,
    bind_layout: BindGroupLayout,
}

pub struct TextManagerDescriptor;

pub struct TextInstance {
    bind: BindGroup,
    uniform: wgpu::Buffer,
    texture: Texture,
    sampler: Sampler,
}

impl Default for TextDescriptor<'_> {
    fn default() -> Self {
        Self {
            text: Default::default(),
            rect: Rectangle::new(0, 0, 200, 24),
            metrics: Metrics::new(24.0, 20.0),
            order: 100,
            visible: true,
        }
    }
}

impl Element for Text {}
impl Element for TextManager {}
impl Element for TextInstance {}

impl Descriptor for TextManagerDescriptor {
    type Target = TextManager;

    fn build(self, world: &World) -> Self::Target {
        let mut font_system = FontSystem::new();
        let database = font_system.db_mut();

        let sans = include_bytes!("../../fonts/SourceHanSansCN-Regular.otf").to_vec();
        let serif = include_bytes!("../../fonts/SourceHanSerifCN-Regular.otf").to_vec();

        database.load_font_data(sans);
        database.load_font_data(serif);

        let swash_cache = SwashCache::new();

        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single_fetch::<ViewportManager>().unwrap();

        let caps = render.surface.get_capabilities(&render.adapter);
        let format = *caps.formats.first().unwrap();

        let shader_vs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("vertex_shader"),
            source: ShaderSource::Wgsl(include_str!("vertex.wgsl").into()),
        });

        let shader_fs = render.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });

        let bind_layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("text_bind_layout"),
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
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = render
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("text_pipeline_layout"),
                bind_group_layouts: &[&viewport.layout, &bind_layout],
                push_constant_ranges: &[],
            });

        let pipeline = render
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("text_pipeline"),
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

        TextManager {
            font_system,
            swash_cache,
            pipeline,
            bind_layout,
        }
    }
}

impl Descriptor for TextDescriptor<'_> {
    type Target = Text;

    fn build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();
        let viewport = world.single::<ViewportInstance>().unwrap();
        let manager = &mut *world.single_fetch_mut::<TextManager>().unwrap();

        // instance //

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("text_uniform"),
            contents: bytemuck::bytes_of(&VertexUniform {
                origin: self.rect.origin.into_array(),
                extend: self.rect.extend.into_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let upscale_metrics = self.metrics.scale(4.0);
        let upscale_width = self.rect.width() * 4;
        let upscale_height = self.rect.height() * 4;

        let mut data = vec![0; (upscale_width * upscale_height * 4) as usize];

        let mut buffer = cosmic_text::Buffer::new(&mut manager.font_system, upscale_metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut manager.font_system);

        let attrs = Attrs::new().family(Family::Name("Source Han Sans CN"));
        buffer_borrow.set_size(Some(upscale_width as f32), Some(upscale_height as f32));
        buffer_borrow.set_text(self.text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);
        buffer_borrow.draw(
            &mut manager.swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * upscale_width as i32) * 4) as usize;
                let rgba = color.as_rgba();
                if start >= data.len() {
                    return;
                }
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        let texture = render.device.create_texture_with_data(
            &render.queue,
            &TextureDescriptor {
                label: Some("text_texture"),
                size: Extent3d {
                    width: upscale_width,
                    height: upscale_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            &data,
        );

        let view = texture.create_view(&TextureViewDescriptor {
            label: Some("text_view"),
            ..Default::default()
        });

        let sampler = render.device.create_sampler(&SamplerDescriptor {
            label: Some("text_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("text_bind"),
            layout: &manager.bind_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &uniform,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let instance = world.insert(TextInstance {
            bind,
            uniform,
            texture,
            sampler,
        });

        let control = world.insert(RenderControl {
            visible: self.visible,
            order: self.order,
        });

        world.observer(control, move |Redraw, world, _| {
            let manager = world.single_fetch::<TextManager>().unwrap();
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

        Text {
            instance,
            cmd: world.commander(),
        }
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        let instance = self.instance;
        self.cmd.queue(move |world| {
            world.remove(instance);
        });
    }
}

#[cfg(false)]
pub struct TextEdit {
    inner: Painter,
    editor: Editor<'static>,
    font_system: Arc<Mutex<FontSystem>>,
    swash_cache: Arc<Mutex<SwashCache>>,

    redraw: bool,
}

#[cfg(false)]
impl Element for TextEdit {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(PointerCollider {
            rect: self.inner.get_rect(),
            z_order: self.inner.get_z_order(),
        });

        world.dependency(collider, this);

        world.observer(collider, move |&PointerHit(event), world, _| match event {
            PointerEvent::Pressed(position) => {
                let fetched = &mut *world.fetch_mut(this).unwrap();

                let point = position - fetched.inner.get_rect().left_up();
                let point = Position::new(point.x, -point.y);

                let mut font_system = fetched.font_system.lock();
                fetched.editor.action(
                    &mut font_system,
                    Action::Click {
                        x: point.x,
                        y: point.y,
                    },
                );

                drop(font_system);

                let focus = world.single::<Focus>().unwrap();
                world.trigger(focus, RequestFocus(Some(this.untyped())));

                fetched.redraw = true;
            }
            PointerEvent::Moved(position) => {
                let fetched = &mut *world.fetch_mut(this).unwrap();

                let point = position - fetched.inner.get_rect().left_up();
                let point = Position::new(point.x, -point.y);

                let mut font_system = fetched.font_system.lock();
                fetched.editor.action(
                    &mut font_system,
                    Action::Drag {
                        x: point.x,
                        y: point.y,
                    },
                );

                drop(font_system);

                fetched.redraw = true;
            }
            _ => {}
        });

        world.observer(this, |FocusInput(event), world, this| {
            if !event.state.is_pressed() {
                return;
            }

            let fetched = &mut *world.fetch_mut(this).unwrap();

            let mut font_system = fetched.font_system.lock();
            let mut editor = fetched.editor.borrow_with(&mut font_system);

            let modifiers = world.single_fetch::<LnwinModifiers>().unwrap();
            let ctrl_down = modifiers.0.state().control_key();
            let shift_down = modifiers.0.state().shift_key();

            if shift_down && let Selection::None = editor.selection() {
                let cursor = editor.cursor();
                editor.set_selection(Selection::Normal(cursor));
            }

            match &event.logical_key {
                Key::Named(NamedKey::ArrowLeft) if ctrl_down => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::LeftWord))
                }
                Key::Named(NamedKey::ArrowRight) if ctrl_down => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::RightWord))
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Left));
                }
                Key::Named(NamedKey::ArrowRight) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Right));
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Up));
                }
                Key::Named(NamedKey::ArrowDown) => {
                    if !shift_down {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(Action::Motion(Motion::Down));
                }
                Key::Named(NamedKey::Home) => editor.action(Action::Motion(Motion::Home)),
                Key::Named(NamedKey::End) => editor.action(Action::Motion(Motion::End)),
                Key::Named(NamedKey::PageUp) => editor.action(Action::Motion(Motion::PageUp)),
                Key::Named(NamedKey::PageDown) => editor.action(Action::Motion(Motion::PageDown)),
                Key::Named(NamedKey::Escape) => editor.action(Action::Escape),
                Key::Named(NamedKey::Enter) => {
                    editor.delete_selection();
                    editor.action(Action::Enter);
                }
                Key::Named(NamedKey::Backspace) if ctrl_down => {
                    if !editor.delete_selection() {
                        let cursor = editor.cursor();
                        editor.set_selection(Selection::Normal(cursor));
                        editor.action(Action::Motion(Motion::PreviousWord));
                        editor.delete_selection();
                        editor.set_selection(Selection::None);
                    }
                }
                Key::Named(NamedKey::Delete) if ctrl_down => {
                    if !editor.delete_selection() {
                        let cursor = editor.cursor();
                        editor.set_selection(Selection::Normal(cursor));
                        editor.action(Action::Motion(Motion::NextWord));
                        editor.delete_selection();
                        editor.set_selection(Selection::None);
                    }
                    let cursor = editor.cursor();
                    editor.set_selection(Selection::Normal(cursor));
                    editor.action(Action::Motion(Motion::NextWord));
                    editor.delete_selection();
                    editor.set_selection(Selection::None);
                }
                Key::Named(NamedKey::Backspace) => {
                    if !editor.delete_selection() {
                        editor.action(Action::Backspace);
                    }
                }
                Key::Named(NamedKey::Delete) => {
                    if !editor.delete_selection() {
                        editor.action(Action::Delete);
                    }
                }
                Key::Named(key) => {
                    if let Some(text) = key.to_text() {
                        editor.delete_selection();
                        for c in text.chars() {
                            editor.action(Action::Insert(c));
                        }
                    }
                }
                Key::Character(text) => {
                    editor.delete_selection();
                    for c in text.chars() {
                        editor.action(Action::Insert(c));
                    }
                }
                _ => {}
            }

            drop(font_system);

            fetched.redraw = true;
        });

        world.observer(collider, move |&PointerMenu(position), world, _| {
            world.insert(world.build(MenuDescriptor {
                position,
                entries: vec![MenuEntryDescriptor {
                    label: "Remove".into(),
                    action: Box::new(move |world| {
                        world.remove(this);
                    }),
                }],
                ..Default::default()
            }));
        });

        let interface = world.single::<Interface>().unwrap();

        let tracker = world.observer(interface, move |Redraw, world, _| {
            let mut this = world.fetch_mut(this).unwrap();

            if this.redraw {
                this.redraw();
            }
        });

        world.dependency(tracker, this);
    }
}

#[cfg(false)]
impl TextEdit {
    pub fn new(
        rect: Rectangle,
        text: String,
        manager: &mut TextManager,
        interface: &mut Interface,
    ) -> TextEdit {
        let mut font_system = manager.font_system.lock();
        let mut swash_cache = manager.swash_cache.lock();

        let metrics = Metrics::new(24.0, 20.0);

        let mut buffer = Buffer::new(&mut font_system, metrics);
        let mut buffer_borrow = buffer.borrow_with(&mut font_system);

        let attrs = Attrs::new().family(Family::Name("Source Han Sans CN"));
        buffer_borrow.set_size(Some(rect.width() as f32), Some(rect.height() as f32));
        buffer_borrow.set_text(&text, &attrs, Shaping::Advanced);
        buffer_borrow.shape_until_scroll(true);

        let mut data = vec![0; (rect.width() * rect.height() * 4) as usize];

        buffer_borrow.draw(
            &mut swash_cache,
            Color::rgb(0xFF, 0xFF, 0xFF),
            |x, y, _, _, color| {
                let start = ((x + y * rect.width() as i32) * 4) as usize;
                let rgba = color.as_rgba();
                data[start] = rgba[0];
                data[start + 1] = rgba[1];
                data[start + 2] = rgba[2];
                data[start + 3] = rgba[3];
            },
        );

        let inner = Painter::new_with(rect, data, interface);

        TextEdit {
            inner,
            editor: Editor::new(buffer),
            font_system: manager.font_system.clone(),
            swash_cache: manager.swash_cache.clone(),
            redraw: false,
        }
    }

    pub fn clone_text(&self) -> String {
        self.editor.with_buffer(|buffer| {
            let mut selection = String::new();

            if let Some(line) = buffer.lines.first() {
                selection.push_str(line.text());
                for line_i in 1..buffer.lines.len() {
                    selection.push('\n');
                    selection.push_str(buffer.lines[line_i].text());
                }
            }

            selection
        })
    }

    fn redraw(&mut self) {
        self.redraw = false;

        let mut font_system = self.font_system.lock();
        let mut swash_cache = self.swash_cache.lock();

        let mut writer = self.inner.open_writer();
        writer.clear([0; 4]);
        self.editor.shape_as_needed(&mut font_system, true);
        self.editor.draw(
            &mut font_system,
            &mut swash_cache,
            Color::rgba(255, 255, 255, 255),
            Color::rgba(255, 255, 255, 127),
            Color::rgba(127, 127, 255, 127),
            Color::rgba(255, 255, 255, 255),
            |x, y, w, h, color| {
                let rgba = color.as_rgba();
                for x in x..(x + w as i32) {
                    for y in y..(y + h as i32) {
                        writer.draw(x, y, rgba);
                    }
                }
            },
        );
    }
}