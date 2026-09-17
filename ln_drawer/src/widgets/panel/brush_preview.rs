use std::sync::Arc;

use glam::{DVec2, I64Vec2, IVec2, UVec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    layer::{
        ChunkPool, DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_DISABLED, Layer, LayerPipeline,
        brush::{BrushParams, Draw, DrawPipeline, param::BrushParam, round::RoundBrush},
        wrapper::{BrushConfigurationChanged, LayerWrapper},
    },
    measures::{FI64Ext, Rectangle},
    render::{
        Render, RenderControl,
        camera::{Camera, CameraBind, CameraDescriptor},
    },
    widgets::renderer::texture_quad::TextureQuad,
};

/// Where a [`BrushPreview`] takes its brush from.
#[derive(Clone, Copy)]
pub enum PreviewSource {
    /// The temporary working brush owned by [`LayerWrapper`]; refreshed on every
    /// [`BrushConfigurationChanged`].
    Active,
    /// A fixed preset of the registry. Immutable, so it is generated once.
    Preset(usize),
}

/// Bakes a brush stroke with the real engine and renders it into a [`TextureQuad`]'s target.
///
/// The bake uses a dedicated [`DrawPipeline`] so it never touches the live stroke or undo state of
/// [`LayerWrapper::brush`].
pub struct BrushPreviewGenerator {
    layer: Arc<LayerPipeline>,
    pipe: DrawPipeline,
    camera_layout: wgpu::BindGroupLayout,
}

impl BrushPreviewGenerator {
    pub fn new(world: &World) -> Self {
        let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
        let layer = wrapper.brush.layer.clone();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        BrushPreviewGenerator {
            pipe: DrawPipeline::new(layer.clone()),
            layer,
            camera_layout: camera_bind.layout.clone(),
        }
    }

    /// Bake `brush` over a fixed seed pattern and render the result into `target`.
    pub fn generate(
        &mut self,
        render: &Render,
        target: &TextureQuad,
        brush: &dyn BrushParams,
        debug: bool,
    ) {
        let chunk = DEFAULT_CHUNK_SIZE;
        let full = Rectangle::new_extend(0, 0, chunk, chunk);

        let mut dst = Layer {
            chunks: HashMap::new(),
            chunk_size: chunk,
            mipmap_levels: DEFAULT_MIPMAP_DISABLED,
            controlled: false,
        };
        let mut pool = ChunkPool {
            list: Vec::new(),
            chunk_size: chunk,
        };
        self.layer.layer_prepare_rect(&mut dst, &mut pool, full);

        // The seed is baked first so that destination-sampling brushes (blur / smudge / tint)
        // have something to work with.
        self.paint_seed(&mut dst, chunk);
        self.paint_stroke(&mut dst, brush);

        let fit = target.size.min_element().max(1) as f64 / chunk as f64;
        let camera = Camera::from_descriptor(
            CameraDescriptor {
                size: target.size,
                center: I64Vec2::q32_from_i32(IVec2::splat((chunk / 2) as i32)),
                zoom: i64::q32_from_f64(fit.log2()),
            },
            render,
            &self.camera_layout,
        );

        target.render_layer(render, &self.layer, &dst, &camera, debug);
    }

    /// A serpentine of coloured bands filling the chunk, used as the preview's background.
    fn paint_seed(&mut self, dst: &mut Layer, chunk: u32) {
        let c = chunk as i32;
        let margin = c / 8;
        let colors = [
            Srgba::new(0.90, 0.20, 0.20, 1.0),
            Srgba::new(0.20, 0.80, 0.30, 1.0),
            Srgba::new(0.25, 0.40, 0.95, 1.0),
            Srgba::new(0.95, 0.85, 0.20, 1.0),
        ];
        let path = [
            IVec2::new(margin, margin),
            IVec2::new(c - margin, margin),
            IVec2::new(c - margin, c / 2),
            IVec2::new(margin, c / 2),
            IVec2::new(margin, c - margin),
            IVec2::new(c - margin, c - margin),
        ];

        for i in 0..path.len() - 1 {
            let brush = RoundBrush {
                size: BrushParam::constant(chunk as f32 * 0.22),
                flow: BrushParam::constant(0.9),
                softness: BrushParam::constant(0.7),
                color: colors[i % colors.len()],
                erase: false,
            };

            if i == 0 {
                self.pipe.draw(
                    dst,
                    &brush,
                    Draw {
                        position: I64Vec2::q32_from_i32(path[0]),
                        force: 1.0,
                    },
                );
            }
            self.pipe.draw(
                dst,
                &brush,
                Draw {
                    position: I64Vec2::q32_from_i32(path[i + 1]),
                    force: 1.0,
                },
            );
        }

        self.pipe.submit(dst, None);
    }

    /// A gentle S-curve with a pressure ramp, drawn with the previewed brush.
    fn paint_stroke(&mut self, dst: &mut Layer, brush: &dyn BrushParams) {
        let c = DEFAULT_CHUNK_SIZE as f32;
        let steps = 24;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = c * (0.18 + 0.64 * t);
            let y = c * (0.5 + 0.28 * (t * std::f32::consts::TAU).sin());

            brush.draw(
                &mut self.pipe,
                dst,
                Draw {
                    position: I64Vec2::q32_from_f64(DVec2::new(x as f64, y as f64)),
                    force: 0.25 + 0.75 * t,
                },
            );
        }

        self.pipe.submit(dst, None);
    }
}

impl Element for BrushPreviewGenerator {}

/// A UI element that keeps a real engine-rendered preview of a brush up to date.
pub struct BrushPreview {
    pub order: isize,
    pub source: PreviewSource,

    quad: Handle<TextureQuad>,
    dirty: bool,
    generated: bool,
}

impl BrushPreview {
    pub fn build(world: &World, source: PreviewSource, size: UVec2, order: isize) -> Handle<Self> {
        let quad = world.insert(TextureQuad::new(world, size, order));

        world.insert(BrushPreview {
            order,
            source,
            quad,
            dirty: false,
            generated: false,
        })
    }

    fn regenerate(&mut self, world: &World) {
        let render = world.single_fetch::<Render>().unwrap();
        let quad = world.fetch(self.quad).unwrap();
        let mut generator = world.single_fetch_mut::<BrushPreviewGenerator>().unwrap();

        match self.source {
            PreviewSource::Active => {
                let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
                generator.generate(&render, &quad, wrapper.active(), wrapper.debug);
            }
            PreviewSource::Preset(index) => {
                let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
                if let Some(preset) = wrapper.brushes.get(index) {
                    generator.generate(&render, &quad, preset.brush.as_ref(), false);
                }
            }
        }
    }
}

impl Element for BrushPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        if let PreviewSource::Active = self.source {
            let wrapper = world.single::<LayerWrapper>().unwrap();
            world.observer(wrapper, move |&BrushConfigurationChanged, world| {
                if let Ok(mut preview) = world.fetch_mut(this) {
                    preview.dirty = true;
                }
                RenderControl::request_redraw(world);
            });
        }

        let order = self.order;
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut preview = world.fetch_mut(this).unwrap();
                if !preview.generated || preview.dirty {
                    preview.dirty = false;
                    preview.regenerate(world);
                }
                None
            })),
            draw: None,
        });
        RenderControl::reorder(Some(order), world, control);

        RenderControl::request_redraw(world);
    }
}
