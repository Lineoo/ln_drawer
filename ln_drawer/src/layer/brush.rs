pub mod blur;
pub mod param;
pub mod pixel;
pub mod round;
pub mod smudge;
pub mod tint;

use std::mem::size_of;

use bytemuck::{Pod, Zeroable, bytes_of};
use glam::{I64Vec2, UVec2};
use palette::Srgba;
use wgpu::{BindGroup, ComputePass};

use crate::{
    layer::{
        DRAWS_ARRAY_CAPACITY, LayerPipeline, PlainLayer, RawLayer, RawLayerChunk, Standalone,
        brush::{
            blur::BlurBrush,
            param::{BrushParam, BrushParamKey, BrushValue, BrushValueMut},
            pixel::PixelBrush,
            round::RoundBrush,
            smudge::SmudgeBrush,
            tint::TintBrush,
        },
        chunks_key_to_rect, rect_chunks_iter, write_dispatch,
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone, Copy)]
pub struct Draw {
    pub position: I64Vec2,
    pub force: f32,
}

pub struct Stroke {
    pub prev: Option<Draw>,
    pub dirty: Rectangle,
    pub replace: bool,
    pub bridge: bool,
}

impl Stroke {
    pub fn new<T: Brush + ?Sized>(brush: &T) -> Self {
        Stroke {
            prev: None,
            dirty: Rectangle::default(),
            replace: brush.replace_mode(),
            bridge: brush.bridge_mode(),
        }
    }

    pub fn batch<T: Brush + ?Sized>(&mut self, brush: &T, target: Draw) -> Option<DrawsBatch> {
        // pre-process
        let target = Draw {
            position: target.position,
            force: target.force.clamp(0.0, 1.0),
        };

        let start = self.prev.is_none();

        let mut draws = Vec::new();
        let (next, dirty, draws_cnt) = brush.processor(self.prev, target, &mut draws);
        self.prev = Some(next);

        if start {
            self.dirty = Rectangle::new_half(next.position.q32_floor(), UVec2::ZERO);
        } else {
            self.dirty = self.dirty.grow(dirty);
        }

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            None
        } else {
            Some(DrawsBatch {
                start,
                draws_cnt,
                draws,
                dirty,
                replace: self.replace,
                bridge: self.bridge,
            })
        }
    }

    /// All finished, merge to dst layer and optionally notify stream thread unsaved chunks
    pub fn submit_layers(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        raw: &RawLayer,
        scratch: &mut PlainLayer,
    ) {
        debug_assert_eq!(raw.chunk_size, scratch.chunk_size);

        write_dispatch(&lp.queue, &lp.dispatch, 0, self.dirty);

        for (chunk_key, scratch_chunk) in &scratch.chunks {
            let chunk_rect = chunks_key_to_rect(*chunk_key, scratch.chunk_size);
            if let Some(RawLayerChunk::Filled(raw_chunk)) = raw.chunks.get(chunk_key) {
                if self.replace {
                    lp.copy_chunk(
                        cpass,
                        scratch_chunk,
                        chunk_rect,
                        &raw_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                } else {
                    lp.merge_chunk_over_read_write(
                        cpass,
                        scratch_chunk,
                        chunk_rect,
                        raw_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                };
            }
        }

        // TODO swap draw
        // for (chunk_key, scratch_chunk) in &scratch.chunks {
        //     let chunk_rect = chunks_key_to_rect(*chunk_key, scratch.chunk_size);
        //     if let Some(RawLayerChunk::Filled(raw_chunk)) = raw.chunks.get(chunk_key)
        //         && let Some(swp_chunk) = self.scratch_swp.chunks.get(chunk_key)
        //     {
        //         if self.replace {
        //             lp.copy_chunk(
        //                 cpass,
        //                 scratch_chunk,
        //                 chunk_rect,
        //                 raw_chunk,
        //                 chunk_rect,
        //                 self.dirty,
        //             );
        //         } else {
        //             cpass.set_pipeline(&lp.merge_pipelines.over);
        //             cpass.set_bind_group(0, Some(&lp.dispatch_group), &[0]);
        //             cpass.set_bind_group(1, Some(&raw_chunk.read), &[]);
        //             cpass.set_bind_group(2, Some(&scratch_chunk.read), &[]);
        //             cpass.set_bind_group(3, Some(&swp_chunk.write), &[]);
        //             dispatch_workgroups(cpass, &[self.dirty, chunk_rect]);

        //             cpass.set_pipeline(&lp.copy_pipeline);
        //             cpass.set_bind_group(0, Some(&lp.dispatch_group), &[0]);
        //             cpass.set_bind_group(1, Some(&raw_chunk.write), &[]);
        //             cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
        //             dispatch_workgroups(cpass, &[self.dirty, chunk_rect]);
        //         };
        //     }
        // }
    }

    /// All finished, merge to dst layer and optionally notify stream thread unsaved chunks
    pub fn submit_standalone(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        raw: &Standalone,
        scratch: &mut Standalone,
    ) {
        write_dispatch(&lp.queue, &lp.dispatch, 0, self.dirty);

        if self.replace {
            lp.copy_chunk(
                cpass,
                &scratch.chunk,
                scratch.rect,
                &raw.chunk,
                raw.rect,
                self.dirty,
            );
        } else {
            lp.merge_chunk_over_read_write(
                cpass,
                &scratch.chunk,
                scratch.rect,
                &raw.chunk,
                raw.rect,
                self.dirty,
            );
        };
    }
}

pub struct DrawsBatch {
    pub start: bool,
    pub draws_cnt: u32,
    pub draws: Vec<u8>,
    pub dirty: Rectangle,
    pub replace: bool,
    pub bridge: bool,
}

impl DrawsBatch {
    pub fn upload(&self, lp: &LayerPipeline, bridge: &mut Standalone) {
        self.upload_dispatch(lp);
        if self.bridge {
            self.upload_bridge(lp, bridge);
        }
    }

    pub fn upload_bridge(&self, lp: &LayerPipeline, bridge: &mut Standalone) {
        bridge.rect = Rectangle::new_extend(
            self.dirty.horizontal_center() - bridge.rect.extend.x as i32 / 2,
            self.dirty.vertical_center() - bridge.rect.extend.y as i32 / 2,
            bridge.rect.extend.x,
            bridge.rect.extend.y,
        );

        write_dispatch(&lp.queue, &bridge.chunk.rectangle, 0, bridge.rect);
    }

    pub fn upload_dispatch(&self, lp: &LayerPipeline) {
        write_dispatch(&lp.queue, &lp.draws_dispatch, 0, self.dirty);
        write_dispatch(&lp.queue, &lp.dispatch, 0, self.dirty);

        (lp.queue).write_buffer(&lp.draws_length, 0, bytes_of(&self.draws_cnt));
        (lp.queue).write_buffer(&lp.draws_array, 0, &self.draws);
    }

    /// must `upload` before draw
    pub fn draw_layers<T: Brush + ?Sized>(
        self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        scratch: &PlainLayer,
        bridge: &Standalone,
        brush: &T,
    ) {
        if self.start {
            brush.prepare_stroke(cpass, lp);
        }

        if self.bridge {
            for key in rect_chunks_iter(bridge.rect, scratch.chunk_size) {
                let scratch_rect = chunks_key_to_rect(key, scratch.chunk_size);
                let Some(scratch_chunk) = scratch.chunks.get(&key) else {
                    continue;
                };

                lp.copy_chunk_direct(
                    cpass,
                    scratch_chunk,
                    scratch_rect,
                    &bridge.chunk,
                    bridge.rect,
                );
            }

            brush.prepare_draw(cpass, lp, &bridge.chunk.read);
            for key in rect_chunks_iter(self.dirty, scratch.chunk_size) {
                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                let Some(chunk) = scratch.chunks.get(&key) else {
                    continue;
                };

                lp.draw_chunk_cross(
                    cpass,
                    &bridge.chunk,
                    bridge.rect,
                    chunk,
                    chunk_rect,
                    brush,
                    self.dirty,
                );
            }
        } else {
            for key in rect_chunks_iter(self.dirty, scratch.chunk_size) {
                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                let Some(chunk) = scratch.chunks.get(&key) else {
                    continue;
                };
                brush.prepare_draw(cpass, lp, &chunk.read);
                lp.draw_chunk_read_write(cpass, chunk, chunk_rect, brush, self.dirty);
            }
        }
    }

    /// must `upload` before draw
    pub fn draw_standalone<T: Brush + ?Sized>(
        self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        scratch: &Standalone,
        brush: &T,
    ) {
        if self.start {
            brush.prepare_stroke(cpass, lp);
        }

        brush.prepare_draw(cpass, lp, &scratch.chunk.read);
        lp.draw_chunk_read_write(cpass, &scratch.chunk, scratch.rect, brush, self.dirty);
    }
}

pub trait Brush {
    fn processor(&self, from: Option<Draw>, to: Draw, out: &mut Vec<u8>) -> (Draw, Rectangle, u32);

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

pub trait BrushParams: Brush + Send {
    fn dup(&self) -> Box<dyn BrushParams>;

    #[expect(dead_code)]
    fn list_params(&self) -> &'static [BrushParamKey];

    fn param(&self, key: BrushParamKey) -> Option<BrushValue<'_>>;

    fn param_mut(&mut self, key: BrushParamKey) -> Option<BrushValueMut<'_>>;
}

trait DrawsProcessor {
    type Draw: Clone + Copy + Pod + Zeroable;

    fn process(&self, draw: Draw) -> Self::Draw;
    fn step(&self, draw: Draw) -> f32;
    fn dirty(&self, draw: Self::Draw) -> Rectangle;

    fn processor_output(
        &self,
        from: Option<Draw>,
        to: Draw,
        out: &mut Vec<u8>,
    ) -> (Draw, Rectangle, u32) {
        let Some(from) = from else {
            let processed = self.process(to);
            let dirty = self.dirty(processed);
            out.extend_from_slice(bytes_of(&processed));
            return (to, dirty, 1);
        };

        let from_position = from.position.q32_as_f64();
        let to_position = to.position.q32_as_f64();
        let whole_dist = from_position.distance(to_position);
        let limit = DRAWS_ARRAY_CAPACITY as usize - size_of::<Self::Draw>();

        let mut cnt = 0;
        let mut curr = from;
        let mut curr_position = from_position;
        let mut dirty = Rectangle::new_half(from.position.q32_as_i32(), UVec2::ZERO);
        loop {
            let step = self.step(curr) as f64;
            if curr_position.distance(to_position) < step || out.len() > limit {
                break;
            }

            curr_position = curr_position.move_towards(to_position, step);
            curr.position = I64Vec2::q32_from_f64(curr_position);
            let curr_dist = curr_position.distance(to_position);
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - (curr_dist / whole_dist) as f32,
            };
            curr.force = (1.0 - progress) * from.force + progress * to.force;

            let processed = self.process(curr);
            dirty = dirty.grow(self.dirty(processed));
            out.extend_from_slice(bytes_of(&processed));
            cnt += 1;
        }

        (curr, dirty, cnt)
    }
}

macro_rules! brush_params {
    ($ty:ty, { $( $key:ident => $variant:ident($field:ident) ),* $(,)? }) => {
        impl $crate::layer::brush::BrushParams for $ty {
            fn dup(&self) -> Box<dyn $crate::layer::brush::BrushParams> {
                Box::new(self.clone())
            }

            fn list_params(&self) -> &'static [$crate::layer::brush::param::BrushParamKey] {
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
    Spacing => Scalar(spacing),
    Color => Color(color),
    Erase => Toggle(erase),
});

brush_params!(PixelBrush, {
    Size => Scalar(size),
    Color => Color(color),
    Erase => Toggle(erase),
});

brush_params!(BlurBrush, {
    Size => Scalar(size),
    Sigma => Scalar(sigma),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
});

brush_params!(SmudgeBrush, {
    Size => Scalar(size),
    Flow => Scalar(flow),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
    Color => Color(color),
    ColorRatio => Scalar(color_ratio),
    SampleRadius => Scalar(sample_radius),
    SampleRate => Scalar(sample_rate),
});

brush_params!(TintBrush, {
    Size => Scalar(size),
    Softness => Scalar(softness),
    Spacing => Scalar(spacing),
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
