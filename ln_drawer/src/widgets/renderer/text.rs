use std::{collections::HashMap, mem::size_of};

use cosmic_text::{Align, Attrs, Buffer as CosmicBuffer, CacheKey, FontSystem, Metrics, Shaping};
use glam::{I64Vec2, UVec2, Vec4};
use ln_world::{Element, Handle, World};
use smallvec::SmallVec;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::{
    layout::val::{Val, ValChanged},
    lnwin::Lnwindow,
    measures::{FI64Ext, IRect, URect},
    render::{
        MSAA_STATE, Render, RenderDrawControl, RenderPrepareControl,
        camera::{Camera, CameraBind},
    },
    widgets::{common::WidgetCommon, shaders::shader_compile},
};

mod atlas;
mod raster;

use atlas::{GLYPH_ATLAS_SIZE, GLYPH_PADDING, GlyphAtlasPage};
use raster::{GlyphKind, GlyphRasterizer, SwashRasterizer};

pub struct Text {
    pub text: String,
    pub rect: Handle<Val<IRect>>,
    pub attr: TextAttr,
    pub order: isize,
    pub common: WidgetCommon,
    pub modulate: Handle<Val<Vec4>>,
    /// will delay text draw to next render prepare phase
    pub outdated: bool,
}

pub struct TextAttr {
    pub metrics: Metrics,
    pub attrs: Attrs<'static>,
    pub align: Align,
    pub extra_upscale: f32,
}

pub struct TextPipeline {
    font_system: FontSystem,
    rasterizer: Box<dyn GlyphRasterizer + Send>,

    glyphs: HashMap<CacheKey, Option<GlyphSlot>>,
    pages: Vec<GlyphAtlasPage>,
    epoch: u64,
    reset_pending: bool,

    render_pipeline: RenderPipeline,
    material_layout: BindGroupLayout,
    atlas_layout: BindGroupLayout,
    sampler: Sampler,
}

pub struct SetText(pub String);

#[derive(Clone, Copy)]
struct GlyphSlot {
    page: usize,
    kind: GlyphKind,
    uv: [f32; 4],
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextMaterial {
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphInstance {
    origin: [f32; 2],
    size: [f32; 2],
    uv: [f32; 4],
    kind: u32,
    _pad: u32,
}

#[derive(Clone, Copy)]
struct GlyphRun {
    page: usize,
    start: u32,
    count: u32,
}

struct TextInstance {
    buffer: CosmicBuffer,
    glyph_buffer: Option<Buffer>,
    glyph_capacity: u64,
    runs: SmallVec<[GlyphRun; 2]>,
    material: Buffer,
    material_bind: BindGroup,
    epoch: u64,
    scale: f32,
}

impl Default for TextAttr {
    fn default() -> Self {
        Self {
            metrics: Metrics::new(24.0, 20.0),
            attrs: Attrs::new(),
            align: Align::Justified,
            extra_upscale: 1.0,
        }
    }
}

impl Text {
    pub fn init(&mut self, world: &World, this: Handle<Text>) {
        world.scope("text");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);
        world.dependency(this, self.modulate);

        let lnwindow = world.single_fetch::<Lnwindow>();
        let modulate = world.fetch(self.modulate).val;
        let upscale = self.upscale(&lnwindow);
        let metrics = self.attr.metrics.scale(upscale);

        let render = world.single_fetch::<Render>();
        let mut pipeline = world.single_fetch_mut::<TextPipeline>();
        let buffer = CosmicBuffer::new(&mut pipeline.font_system, metrics);

        let material = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("text_material"),
            contents: bytemuck::bytes_of(&TextMaterial {
                color: modulate.to_array(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let material_bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("text_material"),
            layout: &pipeline.material_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: material.as_entire_binding(),
            }],
        });
        drop(pipeline);

        let instance = world.insert(TextInstance {
            buffer,
            glyph_buffer: None,
            glyph_capacity: 0,
            runs: SmallVec::new(),
            material,
            material_bind,
            epoch: 0,
            scale: upscale,
        });

        world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let mut this = world.fetch_mut(this);
                this.prepare(world, instance);
                None
            }),
        });

        let rect = self.rect;
        world.insert(RenderDrawControl {
            common: self.common,
            order: self.order,
            draw: Box::new(move |world, rpass, extra| {
                let instance = world.fetch(instance);
                let pipeline = world.single_fetch::<TextPipeline>();

                if instance.runs.is_empty() {
                    return;
                }

                let Some(glyph_buffer) = &instance.glyph_buffer else {
                    return;
                };

                let rect = world.fetch(rect).val;
                let lnwindow = world.single_fetch::<Lnwindow>();
                let clip = screen_scissor(rect, extra.camera, &lnwindow);
                let Some(clip) = extra.scissor.intersect(clip) else {
                    return;
                };

                let (start, end) = extra.diagnosis.assign("main > text");
                extra.diagnosis.write(rpass, start);

                rpass.set_pipeline(&pipeline.render_pipeline);
                rpass.set_bind_group(0, &extra.camera.bind, &[]);
                rpass.set_bind_group(1, &instance.material_bind, &[]);
                rpass.set_scissor_rect(clip.origin.x, clip.origin.y, clip.extend.x, clip.extend.y);
                rpass.set_vertex_buffer(0, glyph_buffer.slice(..));

                for run in &instance.runs {
                    let Some(page) = pipeline.pages.get(run.page) else {
                        continue;
                    };
                    rpass.set_bind_group(2, &page.bind, &[]);
                    rpass.draw(0..4, run.start..run.start + run.count);
                }

                rpass.set_scissor_rect(
                    extra.scissor.origin.x,
                    extra.scissor.origin.y,
                    extra.scissor.extend.x,
                    extra.scissor.extend.y,
                );

                extra.diagnosis.write(rpass, end);
            }),
        });

        world.observer(self.rect, move |ValChanged, world| {
            let mut this = world.fetch_mut(this);
            this.outdated = true;

            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        });

        let modulate = self.modulate;
        world.observer(modulate, move |ValChanged, world| {
            let modulate = world.fetch(modulate).val;
            let instance = world.fetch(instance);
            let render = world.single_fetch::<Render>();

            render.queue.write_buffer(
                &instance.material,
                0,
                bytemuck::bytes_of(&TextMaterial {
                    color: modulate.to_array(),
                }),
            );

            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        });

        world.observer(this, move |SetText(text), world| {
            let mut this = world.fetch_mut(this);
            this.set_text(&text[..]);

            let lnwindow = world.single_fetch::<Lnwindow>();
            lnwindow.window.request_redraw();
        });

        self.outdated = true;
    }

    fn prepare(&mut self, world: &World, instance: Handle<TextInstance>) {
        let visible = world.fetch(self.common.visible).val;
        if !visible {
            return;
        }

        let lnwindow = world.single_fetch::<Lnwindow>();
        let upscale = self.upscale(&lnwindow);
        let rect = world.fetch(self.rect).val;
        let render = world.single_fetch::<Render>();

        let mut instance = world.fetch_mut(instance);
        let mut pipeline = world.single_fetch_mut::<TextPipeline>();

        if instance.epoch != pipeline.epoch || instance.scale != upscale {
            instance.scale = upscale;
            self.outdated = true;
        }

        if !self.outdated {
            return;
        }
        self.outdated = false;

        let size = rect.extend.as_vec2() * upscale;
        let metrics = self.attr.metrics.scale(upscale);

        let mut keys: Vec<(CacheKey, i32, i32)> = Vec::new();
        let mut buffer_font = instance.buffer.borrow_with(&mut pipeline.font_system);
        buffer_font.set_metrics(metrics);
        buffer_font.set_size(Some(size.x), Some(size.y));
        buffer_font.set_text(
            &self.text,
            &self.attr.attrs,
            Shaping::Advanced,
            Some(self.attr.align),
        );
        buffer_font.shape_until_scroll(true);
        drop(buffer_font);

        for run in instance.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0., 0.), 1.0);
                keys.push((
                    physical.cache_key,
                    physical.x,
                    run.line_y as i32 + physical.y,
                ));
            }
        }

        let mut glyphs = Vec::with_capacity(keys.len());
        let mut runs: SmallVec<[GlyphRun; 2]> = SmallVec::new();

        for (key, x, line_y) in keys {
            let Some(slot) = pipeline.glyph_slot(&render, key) else {
                continue;
            };

            let left = x + slot.left;
            let top = line_y - slot.top;
            let origin = [
                rect.origin.x as f32 + left as f32 / upscale,
                rect.origin.y as f32 + rect.extend.y as f32
                    - (top + slot.height as i32) as f32 / upscale,
            ];
            let size = [slot.width as f32 / upscale, slot.height as f32 / upscale];

            let index = glyphs.len() as u32;
            glyphs.push(GlyphInstance {
                origin,
                size,
                uv: slot.uv,
                kind: slot.kind.as_u32(),
                _pad: 0,
            });

            match runs.last_mut() {
                Some(run) if run.page == slot.page => run.count += 1,
                _ => runs.push(GlyphRun {
                    page: slot.page,
                    start: index,
                    count: 1,
                }),
            }
        }

        instance.upload(&render, &glyphs, runs);
        instance.epoch = pipeline.epoch;

        if pipeline.reset_pending {
            lnwindow.window.request_redraw();
        }
    }

    pub fn set_text(&mut self, text: impl Into<String> + PartialEq<String>) {
        if text != self.text {
            self.outdated = true;
            self.text = text.into();
        }
    }

    fn upscale(&self, lnwindow: &Lnwindow) -> f32 {
        self.attr.extra_upscale * lnwindow.window.scale_factor() as f32
    }
}

impl TextInstance {
    fn upload(&mut self, render: &Render, glyphs: &[GlyphInstance], runs: SmallVec<[GlyphRun; 2]>) {
        self.runs = runs;

        if glyphs.is_empty() {
            return;
        }

        let bytes: &[u8] = bytemuck::cast_slice(glyphs);

        if self.glyph_capacity < bytes.len() as u64 {
            let buffer = render.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("text_glyphs"),
                contents: bytes,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            self.glyph_buffer = Some(buffer);
            self.glyph_capacity = bytes.len() as u64;
        } else if let Some(buffer) = &self.glyph_buffer {
            render.queue.write_buffer(buffer, 0, bytes);
        }
    }
}

impl TextPipeline {
    pub fn from_world(world: &World) -> Self {
        let render = world.single_fetch::<Render>();
        let camera = world.single_fetch::<CameraBind>();
        let device = &render.device;

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: ShaderSource::Wgsl(shader_compile(include_str!("text.wgsl"), &[]).into()),
        });

        let material_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("text_material_layout"),
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

        let atlas_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("text_atlas_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera.layout),
                Some(&material_layout),
                Some(&atlas_layout),
            ],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(VertexBufferLayout {
                    array_stride: size_of::<GlyphInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                        VertexAttribute {
                            format: VertexFormat::Uint32,
                            offset: 32,
                            shader_location: 3,
                        },
                    ],
                })],
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
                    format: render.target_format(),
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: None,
            multisample: MSAA_STATE,
            multiview_mask: None,
            cache: render.cache.as_ref(),
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("text_atlas"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        Self {
            font_system: Self::font_system(),
            rasterizer: Box::new(SwashRasterizer::new()),
            glyphs: HashMap::new(),
            pages: Vec::new(),
            epoch: 0,
            reset_pending: false,
            render_pipeline,
            material_layout,
            atlas_layout,
            sampler,
        }
    }

    fn font_system() -> FontSystem {
        let mut font_system = FontSystem::new();
        let database = font_system.db_mut();

        // cosmic text cannot find system fonts on android, so we load them by ourselves
        #[cfg(target_os = "android")]
        {
            let sans_reg = include_bytes!("../../../fonts/SourceHanSansCN-Regular.otf").to_vec();
            let serif_reg = include_bytes!("../../../fonts/SourceHanSerifCN-Regular.otf").to_vec();
            let sans_bld = include_bytes!("../../../fonts/SourceHanSansCN-Bold.otf").to_vec();
            let serif_bld = include_bytes!("../../../fonts/SourceHanSerifCN-Bold.otf").to_vec();

            database.load_font_data(sans_reg);
            database.load_font_data(serif_reg);
            database.load_font_data(sans_bld);
            database.load_font_data(serif_bld);
        }

        // default fonts
        database.set_monospace_family("Noto Sans Mono");
        database.set_sans_serif_family("Source Han Sans CN");
        database.set_serif_family("Source Han Serif CN");

        font_system
    }

    fn maintain(&mut self) {
        if !self.reset_pending {
            return;
        }

        self.reset_pending = false;
        self.pages.clear();
        self.glyphs.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn glyph_slot(&mut self, render: &Render, key: CacheKey) -> Option<GlyphSlot> {
        if let Some(slot) = self.glyphs.get(&key) {
            return *slot;
        }

        let Some(raster) = self.rasterizer.rasterize(&mut self.font_system, key) else {
            self.glyphs.insert(key, None);
            return None;
        };

        if raster.width == 0 || raster.height == 0 {
            self.glyphs.insert(key, None);
            return None;
        }

        let kind = raster.kind;
        let mut found = None;

        for (page, atlas) in self.pages.iter_mut().enumerate() {
            if atlas.kind != kind {
                continue;
            }

            if let Some(place) = atlas.insert(&render.queue, &raster) {
                found = Some((page, place));
                break;
            }
        }

        if found.is_none() && self.page_count(kind) < Self::max_pages(kind) {
            let page = GlyphAtlasPage::new(kind, render, &self.atlas_layout, &self.sampler);
            self.pages.push(page);

            let page = self.pages.len() - 1;
            if let Some(place) = self.pages[page].insert(&render.queue, &raster) {
                found = Some((page, place));
            }
        }

        let Some((page, place)) = found else {
            if raster.width + GLYPH_PADDING * 2 > GLYPH_ATLAS_SIZE
                || raster.height + GLYPH_PADDING * 2 > GLYPH_ATLAS_SIZE
            {
                log::error!(
                    "glyph is too large for text atlas: {}x{}",
                    raster.width,
                    raster.height
                );
            } else {
                self.reset_pending = true;
                log::warn!("text glyph atlas is full");
            }

            self.glyphs.insert(key, None);
            return None;
        };

        let slot = GlyphSlot {
            page,
            kind,
            uv: place.uv,
            left: place.left,
            top: place.top,
            width: place.width,
            height: place.height,
        };
        self.glyphs.insert(key, Some(slot));
        Some(slot)
    }

    fn page_count(&self, kind: GlyphKind) -> usize {
        self.pages.iter().filter(|page| page.kind == kind).count()
    }

    fn max_pages(kind: GlyphKind) -> usize {
        match kind {
            GlyphKind::Mask => 2,
            GlyphKind::Color => 1,
        }
    }
}

fn screen_scissor(rect: IRect, camera: &Camera, lnwindow: &Lnwindow) -> URect {
    let size = lnwindow.window.surface_size();
    let left_up =
        lnwindow.screen_to_cursor(camera.world_to_screen(I64Vec2::q32_from_i32(rect.left_up())));
    let right_down =
        lnwindow.screen_to_cursor(camera.world_to_screen(I64Vec2::q32_from_i32(rect.right_down())));

    let x = (left_up.x.max(0.0) as u32).min(size.width);
    let y = (left_up.y.max(0.0) as u32).min(size.height);
    let right = (right_down.x.max(0.0) as u32).min(size.width);
    let down = (right_down.y.max(0.0) as u32).min(size.height);

    URect {
        origin: UVec2::new(x, y),
        extend: UVec2::new(right.saturating_sub(x), down.saturating_sub(y)),
    }
}

impl Element for Text {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}

impl Element for TextPipeline {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        world.insert(RenderPrepareControl {
            prepare: Box::new(|world| {
                world.single_fetch_mut::<TextPipeline>().maintain();
                None
            }),
        });
    }
}

impl Element for TextInstance {}
