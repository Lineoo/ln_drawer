pub mod blur;
pub mod param;
pub mod pixel;
pub mod round;
pub mod smudge;
pub mod tint;

use std::{
    mem::size_of,
    sync::{Arc, mpsc::Sender},
};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{I64Vec2, UVec2};
use hashbrown::HashMap;
use palette::Srgba;
use wgpu::{BindGroup, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, RenderPass};

use crate::{
    layer::{
        Chunk, ChunkPool, DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_DISABLED, DRAWS_ARRAY_CAPACITY, Layer,
        LayerPipeline,
        brush::{
            blur::BlurBrush,
            param::{BrushParam, BrushParamKey, BrushValue, BrushValueMut},
            pixel::PixelBrush,
            round::RoundBrush,
            smudge::SmudgeBrush,
            tint::TintBrush,
        },
        chunk_to_rect, create_chunk, create_chunk_texture, dispatch_workgroups,
        dispatch_workgroups_extend, rect_to_chunks,
        stream::ThreadInput,
        write_dispatch,
    },
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
};

const BRIDGE_CHUNK_SIZE: u32 = 1024;

pub struct DrawPipeline {
    pub layer: Arc<LayerPipeline>,

    scratch_dst: Layer,
    scratch_swp: Layer,
    scratch_pool: ChunkPool,

    bridge: Chunk,

    pub prev: Option<Draw>,
    pub stroke: Option<Stroke>,
}

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: I64Vec2,
    pub force: f32,
}

#[derive(Clone, Copy)]
pub struct Stroke {
    pub dirty: Rectangle,
    pub replace: bool,
    pub bridge: bool,
}

pub trait Brush {
    type Draw: Clone + Copy + Pod + Zeroable;

    fn process(&self, draw: Draw) -> Self::Draw;
    fn step(&self, draw: Self::Draw) -> f32;
    fn dirty(&self, draw: Self::Draw) -> Rectangle;

    /// Append the stamps for the segment `from -> to` (excluding `from`) and
    /// return the position the next segment should resume from.
    ///
    /// The default implementation samples the segment linearly, advancing one
    /// `step` at a time and interpolating `force`. Discrete brushes (e.g. the
    /// pixel brush) override this to emit stamps on their own grid.
    fn interpolate(&self, from: Draw, to: Draw, out: &mut Vec<Self::Draw>) -> Draw {
        let from_position = from.position.q32_as_f64();
        let to_position = to.position.q32_as_f64();
        let whole_dist = from_position.distance(to_position);

        let mut curr = from;
        let mut curr_position = from_position;
        while curr_position.distance(to_position) >= self.step(self.process(curr)) as f64
            && out.len() < DRAWS_ARRAY_CAPACITY as usize / size_of::<Self::Draw>()
        {
            let step = self.step(self.process(curr));
            curr_position = curr_position.move_towards(to_position, step as f64);
            curr.position = I64Vec2::q32_from_f64(curr_position);
            let curr_dist = curr_position.distance(to_position);
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - (curr_dist / whole_dist) as f32,
            };
            curr.force = (1.0 - progress) * from.force + progress * to.force;
            out.push(self.process(curr));
        }

        curr
    }

    fn prepare_stroke(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline) {
        let _ = (cpass, pipeline);
    }

    fn prepare_draw(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline, dst: &BindGroup) {
        let _ = (cpass, pipeline, dst);
    }

    /// - Normal Mode:
    ///     - Destination texture start with __transparent texture__.
    ///     - Render in __over__ mode
    ///     - Merge in __over__ mode
    /// - Replace Mode:
    ///     - Destination texture start with __data from destination layer__.
    ///     - Render in __replace__ mode
    ///     - Merge in __replace__ mode
    fn replace_mode(&self) -> bool;

    /// - Normal Mode:
    ///     - Prepare scratch layer (new/copy depends on replace_mode)
    ///     - Swap draw on scratch layer
    /// - Bridge Mode:
    ///     - Prepare bridge chunk (clear/copy depends on replace_mode)
    ///     - Swap draw on bridge chunk
    ///     - Copy back on scratch layer
    fn bridge_mode(&self) -> bool;

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline);
}

/// Object-safe brush abstraction used by the brush registry and the settings UI.
///
/// It exposes the typed brush fields by [`BrushParamKey`] behind a type-erased borrowed view, so
/// an editor can reach the full [`BrushParam`] (including its [`param::ParamCurve`]) without the
/// brush having to duplicate its fields.
pub trait BrushParams: Send {
    fn draw(&self, pipeline: &mut DrawPipeline, dst: &Layer, draw: Draw);

    /// Produce an independent copy, used for the temporary working brush.
    fn dup(&self) -> Box<dyn BrushParams>;

    #[expect(dead_code)]
    fn param_keys(&self) -> &'static [BrushParamKey];

    fn param(&self, key: BrushParamKey) -> Option<BrushValue<'_>>;
    fn param_mut(&mut self, key: BrushParamKey) -> Option<BrushValueMut<'_>>;
}

macro_rules! brush_params {
    ($ty:ty, { $( $key:ident => $variant:ident($field:ident) ),* $(,)? }) => {
        impl $crate::layer::brush::BrushParams for $ty {
            fn draw(
                &self,
                pipeline: &mut $crate::layer::brush::DrawPipeline,
                dst: &$crate::layer::Layer,
                draw: $crate::layer::brush::Draw,
            ) {
                pipeline.draw(dst, self, draw);
            }

            fn dup(&self) -> Box<dyn $crate::layer::brush::BrushParams> {
                Box::new(self.clone())
            }

            fn param_keys(&self) -> &'static [$crate::layer::brush::param::BrushParamKey] {
                const KEYS: &[$crate::layer::brush::param::BrushParamKey] =
                    &[$( $crate::layer::brush::param::BrushParamKey::$key ),*];
                KEYS
            }

            fn param(
                &self,
                key: $crate::layer::brush::param::BrushParamKey,
            ) -> Option<$crate::layer::brush::param::BrushValue<'_>> {
                use $crate::layer::brush::param::{BrushParamKey, BrushValue};
                match key {
                    $( BrushParamKey::$key => Some(BrushValue::$variant(&self.$field)), )*
                    _ => None,
                }
            }

            fn param_mut(
                &mut self,
                key: $crate::layer::brush::param::BrushParamKey,
            ) -> Option<$crate::layer::brush::param::BrushValueMut<'_>> {
                use $crate::layer::brush::param::{BrushParamKey, BrushValueMut};
                match key {
                    $( BrushParamKey::$key => Some(BrushValueMut::$variant(&mut self.$field)), )*
                    _ => None,
                }
            }
        }
    };
}

brush_params!(RoundBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Softness => Scalar(softness),
    Color => Color(color),
    Erase => Toggle(erase),
});

brush_params!(PixelBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Color => Color(color),
    Erase => Toggle(erase),
});

brush_params!(BlurBrush, {
    Size => Scalar(size),
    Sigma => Scalar(sigma),
    Softness => Scalar(softness),
});

brush_params!(SmudgeBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Softness => Scalar(softness),
    Color => Color(color),
    ColorRatio => Scalar(color_ratio),
    SampleRadius => Scalar(sample_radius),
    SampleRate => Scalar(sample_rate),
});

brush_params!(TintBrush, {
    Size => Scalar(size),
    Softness => Scalar(softness),
    Color => Color(color),
    Flow => Vec4(flow),
});

/// Convenience accessors layered on top of the type-erased reflection interface.
impl dyn BrushParams + '_ {
    pub fn scalar(&self, key: BrushParamKey) -> Option<&BrushParam<f32>> {
        match self.param(key)? {
            BrushValue::Scalar(param) => Some(param),
            _ => None,
        }
    }

    pub fn scalar_mut(&mut self, key: BrushParamKey) -> Option<&mut BrushParam<f32>> {
        match self.param_mut(key)? {
            BrushValueMut::Scalar(param) => Some(param),
            _ => None,
        }
    }

    pub fn color(&self) -> Option<Srgba> {
        match self.param(BrushParamKey::Color)? {
            BrushValue::Color(color) => Some(*color),
            _ => None,
        }
    }

    pub fn set_color(&mut self, value: Srgba) {
        if let Some(BrushValueMut::Color(color)) = self.param_mut(BrushParamKey::Color) {
            *color = value;
        }
    }

    pub fn toggle(&self, key: BrushParamKey) -> Option<bool> {
        match self.param(key)? {
            BrushValue::Toggle(value) => Some(*value),
            _ => None,
        }
    }

    pub fn set_toggle(&mut self, key: BrushParamKey, value: bool) {
        if let Some(BrushValueMut::Toggle(toggle)) = self.param_mut(key) {
            *toggle = value;
        }
    }
}

impl DrawPipeline {
    pub fn new(layer: Arc<LayerPipeline>) -> Self {
        let scratch_dst = Layer {
            chunks: HashMap::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            mipmap_levels: DEFAULT_MIPMAP_DISABLED,
            controlled: false,
        };

        let scratch_swp = Layer {
            chunks: HashMap::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            mipmap_levels: DEFAULT_MIPMAP_DISABLED,
            controlled: false,
        };

        let bridge_texture = create_chunk_texture(&layer.device, BRIDGE_CHUNK_SIZE);
        let bridge = create_chunk(
            &layer.device,
            &layer.chunk_layout,
            bridge_texture,
            chunk_to_rect((0, 0, 0), BRIDGE_CHUNK_SIZE),
        );

        DrawPipeline {
            layer,
            scratch_dst,
            scratch_swp,
            scratch_pool: ChunkPool {
                list: Vec::new(),
                chunk_size: 512,
            },
            bridge,
            prev: None,
            stroke: None,
        }
    }

    /// CPU-end draw process
    pub fn draw<T: Brush>(&mut self, dst: &Layer, brush: &T, target: Draw) {
        let mut draws = Vec::new();

        // draw preprocess
        let target = Draw {
            position: target.position,
            force: target.force.clamp(0.0, 1.0),
        };

        let next = match self.prev {
            Some(prev) => brush.interpolate(prev, target, &mut draws),
            None => {
                draws.push(brush.process(target));
                target
            }
        };

        let mut dirty = Rectangle::new_half(target.position.q32_as_i32(), UVec2::ZERO);
        for &draw in &draws {
            dirty = dirty.grow(brush.dirty(draw));
        }

        self.prev = Some(next);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

        let stroke_start = self.stroke.is_none();
        let stroke = self.stroke.get_or_insert_with(|| Stroke {
            dirty,
            replace: brush.replace_mode(),
            bridge: brush.bridge_mode(),
        });

        stroke.dirty = stroke.dirty.grow(dirty);

        let bridge_rect = Rectangle::new_extend(
            dirty.horizontal_center() - BRIDGE_CHUNK_SIZE as i32 / 2,
            dirty.vertical_center() - BRIDGE_CHUNK_SIZE as i32 / 2,
            BRIDGE_CHUNK_SIZE,
            BRIDGE_CHUNK_SIZE,
        );

        let queue = &self.layer.queue;
        write_dispatch(queue, &self.layer.draws_dispatch, 0, dirty);
        write_dispatch(queue, &self.layer.dispatch, 0, dirty);

        let draw_length = draws.len() as u32;
        queue.write_buffer(&self.layer.draws_length, 0, bytes_of(&draw_length));
        queue.write_buffer(&self.layer.draws_array, 0, cast_slice(&draws));

        if brush.bridge_mode() {
            write_dispatch(&self.layer.queue, &self.bridge.rectangle, 0, bridge_rect);
        }

        let mut encoder = (self.layer.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("layer_draw"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_draw"),
            timestamp_writes: None,
        });

        if stroke_start {
            brush.prepare_stroke(&mut cpass, &self.layer);
        }

        if !self.layer.support_read_write {
            self.draw_upload_swap(&mut cpass, dst, brush, dirty, bridge_rect);
        } else {
            self.draw_upload_read_write(&mut cpass, dst, brush, dirty, bridge_rect);
        }

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);
    }

    fn draw_upload_swap<T: Brush>(
        &mut self,
        cpass: &mut ComputePass,
        dst: &Layer,
        brush: &T,
        dirty: Rectangle,
        bridge_rect: Rectangle,
    ) {
        // prepare

        let reference_layer = match brush.replace_mode() {
            true => Some(dst),
            false => None,
        };

        // Brush always use uncontrolled layer as scratch
        self.layer.prepare_chunks(
            &mut self.scratch_dst,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
            cpass,
        );
        self.layer.prepare_chunks(
            &mut self.scratch_swp,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
            cpass,
        );

        // draw

        if brush.bridge_mode() && brush.replace_mode() {
            let (start, end) = rect_to_chunks(bridge_rect, 0, self.scratch_dst.chunk_size);
            for x in start.0..end.0 {
                for y in start.1..end.1 {
                    let key = (x, y, 0);
                    let src_rect = chunk_to_rect(key, self.scratch_dst.chunk_size);

                    let Some(src_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    // TODO need a extra buffer to represent bridge *sample* rect
                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.bridge.dispatch), &[0]);
                    cpass.set_bind_group(1, Some(&self.bridge.write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[bridge_rect, src_rect]);
                }
            }
        }

        // Bridge brushes share one source texture for the whole batch, so their
        // per-draw preparation only has to run once.
        if brush.bridge_mode() {
            brush.prepare_draw(cpass, &self.layer, &self.bridge.read);
        }

        let (start, end) = rect_to_chunks(dirty, 0, self.scratch_dst.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0);
                let scratch_rect = chunk_to_rect(key, dst.chunk_size);

                if brush.bridge_mode() {
                    let Some(dst_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    brush.set_pipeline(cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&self.bridge.read), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.write), &[]);
                    dispatch_workgroups(cpass, &[dirty, scratch_rect, bridge_rect]);
                } else {
                    let (Some(dst_chunk), Some(swp_chunk)) = (
                        self.scratch_dst.chunks.get(&key),
                        self.scratch_swp.chunks.get(&key),
                    ) else {
                        continue;
                    };

                    brush.prepare_draw(cpass, &self.layer, &dst_chunk.read);

                    brush.set_pipeline(cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.write), &[]);
                    dispatch_workgroups(cpass, &[dirty, scratch_rect]);

                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[dirty, scratch_rect]);
                };
            }
        }
    }

    fn draw_upload_read_write<T: Brush>(
        &mut self,
        cpass: &mut ComputePass,
        dst: &Layer,
        brush: &T,
        dirty: Rectangle,
        bridge_rect: Rectangle,
    ) {
        // prepare

        let reference_layer = match brush.replace_mode() {
            true => Some(dst),
            false => None,
        };

        // Brush always use uncontrolled layer as scratch
        self.layer.prepare_chunks(
            &mut self.scratch_dst,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
            cpass,
        );

        // draw

        if brush.bridge_mode() && brush.replace_mode() {
            let (start, end) = rect_to_chunks(bridge_rect, 0, self.scratch_dst.chunk_size);
            for x in start.0..end.0 {
                for y in start.1..end.1 {
                    let key = (x, y, 0);
                    let src_rect = chunk_to_rect(key, self.scratch_dst.chunk_size);

                    let Some(src_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    // TODO need a extra buffer to represent bridge *sample* rect
                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.bridge.dispatch), &[0]);
                    cpass.set_bind_group(1, Some(&self.bridge.write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[bridge_rect, src_rect]);
                }
            }
        }

        // Bridge brushes share one source texture for the whole batch, so their
        // per-draw preparation only has to run once.
        if brush.bridge_mode() {
            brush.prepare_draw(cpass, &self.layer, &self.bridge.read);
        }

        let (start, end) = rect_to_chunks(dirty, 0, self.scratch_dst.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0);
                let scratch_rect = chunk_to_rect(key, dst.chunk_size);

                if brush.bridge_mode() {
                    let Some(dst_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    brush.set_pipeline(cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&self.bridge.read), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.write), &[]);
                    dispatch_workgroups(cpass, &[dirty, scratch_rect, bridge_rect]);
                } else {
                    let Some(dst_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    brush.prepare_draw(cpass, &self.layer, &self.bridge.read);

                    brush.set_pipeline(cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read_write), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.read_write), &[]);
                    dispatch_workgroups(cpass, &[dirty, scratch_rect]);
                };
            }
        }
    }

    pub fn scratch_render(&self, rpass: &mut RenderPass, camera: &Camera, debug: bool) {
        if let Some(stroke) = &self.stroke {
            self.layer
                .render(&self.scratch_dst, rpass, camera, debug, stroke.replace);
        }
    }

    pub fn request_stream(&mut self, dst: &Layer, tx: &Sender<ThreadInput>) {
        let Some(stroke) = &self.stroke else {
            return;
        };

        for level in 0..dst.mipmap_levels {
            let (start, end) = super::rect_to_chunks(stroke.dirty, level, dst.chunk_size);

            for x in start.0..end.0 {
                for y in start.1..end.1 {
                    let key = (x, y, level);

                    // When any lower chunks are loaded, we then request upper chunks
                    if level > 0
                        && super::lower_chunk_of(key)
                            .iter()
                            .all(|x| !dst.chunks.contains_key(x))
                    {
                        continue;
                    }

                    tx.send(ThreadInput::RequestReal(key)).unwrap();
                }
            }
        }
    }

    /// All finished, merge to dst layer and optionally notify stream thread unsaved chunks
    pub fn submit(&mut self, dst: &mut Layer, tx: Option<&Sender<ThreadInput>>) {
        self.prev = None;
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        debug_assert_eq!(dst.chunk_size, self.scratch_dst.chunk_size);
        debug_assert_eq!(self.scratch_swp.chunk_size, self.scratch_dst.chunk_size);

        let mut encoder = (self.layer.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("layer_submit"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_submit"),
            timestamp_writes: None,
        });

        write_dispatch(&self.layer.queue, &self.layer.dispatch, 0, stroke.dirty);

        // if failed to merge, we simply drop it.
        if tx.is_some() && !self.layer.validate_chunks(dst, stroke.dirty) {
            self.recycle_scratch(&stroke, &mut cpass);
            drop(cpass);
            self.layer.queue.submit([encoder.finish()]);
            return;
        }

        if !self.layer.support_read_write {
            self.submit_upload_swap(dst, &stroke, &mut cpass);
        } else {
            self.submit_upload_read_write(dst, &stroke, &mut cpass);
        }

        // Clear bridge chunk
        if stroke.bridge && stroke.replace {
            cpass.set_pipeline(&self.layer.clear_pipeline);
            cpass.set_bind_group(0, Some(&self.bridge.dispatch), &[0]);
            cpass.set_bind_group(1, Some(&self.bridge.write), &[]);
            dispatch_workgroups_extend(&mut cpass, UVec2::splat(BRIDGE_CHUNK_SIZE));
        }

        self.recycle_scratch(&stroke, &mut cpass);

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);

        self.layer.generate_mipmaps(dst, stroke.dirty);

        if let Some(tx) = tx {
            for level in 0..dst.mipmap_levels {
                let (start, end) = super::rect_to_chunks(stroke.dirty, level, dst.chunk_size);
                for x in start.0..end.0 {
                    for y in start.1..end.1 {
                        tx.send(ThreadInput::MarkUnsaved((x, y, level))).unwrap();
                    }
                }
            }
        }
    }

    fn submit_upload_swap(&mut self, dst: &mut Layer, stroke: &Stroke, cpass: &mut ComputePass) {
        for (src_key, src_chunk) in &self.scratch_dst.chunks {
            let src_rect = chunk_to_rect(*src_key, self.scratch_dst.chunk_size);
            if let Some(dst_chunk) = dst.chunks.get(src_key)
                && let Some(swp_chunk) = self.scratch_swp.chunks.get(src_key)
            {
                if stroke.replace {
                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[stroke.dirty, src_rect]);
                } else {
                    cpass.set_pipeline(&self.layer.merge_pipelines.over);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    cpass.set_bind_group(3, Some(&swp_chunk.write), &[]);
                    dispatch_workgroups(cpass, &[stroke.dirty, src_rect]);

                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[stroke.dirty, src_rect]);
                };
            }
        }
    }

    fn submit_upload_read_write(
        &mut self,
        dst: &mut Layer,
        stroke: &Stroke,
        cpass: &mut ComputePass,
    ) {
        for (src_key, src_chunk) in &self.scratch_dst.chunks {
            let src_rect = chunk_to_rect(*src_key, self.scratch_dst.chunk_size);
            if let Some(dst_chunk) = dst.chunks.get(src_key) {
                if stroke.replace {
                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    dispatch_workgroups(cpass, &[stroke.dirty, src_rect]);
                } else {
                    cpass.set_pipeline(&self.layer.merge_pipelines.over);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read_write), &[]);
                    cpass.set_bind_group(2, Some(&src_chunk.read), &[]);
                    cpass.set_bind_group(3, Some(&dst_chunk.read_write), &[]);
                    dispatch_workgroups(cpass, &[stroke.dirty, src_rect]);
                };
            }
        }
    }

    pub fn discard(&mut self) {
        self.prev = None;
        let Some(stroke) = self.stroke.take() else {
            return;
        };

        let mut encoder = (self.layer.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("layer_discard"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_discard"),
            timestamp_writes: None,
        });

        write_dispatch(&self.layer.queue, &self.layer.dispatch, 0, stroke.dirty);
        self.recycle_scratch(&stroke, &mut cpass);

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);
    }

    /// Need dispatch buffer to be written ahead
    fn recycle_scratch(&mut self, stroke: &Stroke, cpass: &mut ComputePass) {
        if stroke.replace {
            for (key, chunk) in self.scratch_dst.chunks.drain() {
                cpass.set_pipeline(&self.layer.clear_pipeline);
                cpass.set_bind_group(0, Some(&chunk.dispatch), &[0]);
                cpass.set_bind_group(1, Some(&chunk.write), &[]);
                let chunk_rect = chunk_to_rect(key, self.scratch_dst.chunk_size);
                dispatch_workgroups(cpass, &[chunk_rect]);
                self.scratch_pool.list.push(chunk);
            }

            for (key, chunk) in self.scratch_swp.chunks.drain() {
                cpass.set_pipeline(&self.layer.clear_pipeline);
                cpass.set_bind_group(0, Some(&chunk.dispatch), &[0]);
                cpass.set_bind_group(1, Some(&chunk.write), &[]);
                let chunk_rect = chunk_to_rect(key, self.scratch_swp.chunk_size);
                dispatch_workgroups(cpass, &[chunk_rect]);
                self.scratch_pool.list.push(chunk);
            }
        } else {
            for (key, chunk) in self.scratch_dst.chunks.drain() {
                cpass.set_pipeline(&self.layer.clear_pipeline);
                cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                cpass.set_bind_group(1, Some(&chunk.write), &[]);
                let chunk_rect = chunk_to_rect(key, self.scratch_dst.chunk_size);
                dispatch_workgroups(cpass, &[stroke.dirty, chunk_rect]);
                self.scratch_pool.list.push(chunk);
            }

            for (key, chunk) in self.scratch_swp.chunks.drain() {
                cpass.set_pipeline(&self.layer.clear_pipeline);
                cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                cpass.set_bind_group(1, Some(&chunk.write), &[]);
                let chunk_rect = chunk_to_rect(key, self.scratch_swp.chunk_size);
                dispatch_workgroups(cpass, &[stroke.dirty, chunk_rect]);
                self.scratch_pool.list.push(chunk);
            }
        }
    }
}
