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
use wgpu::{BindGroup, CommandEncoderDescriptor, ComputePass, ComputePassDescriptor};

use crate::{
    layer::{
        CacheLayerChunk, CacheLayers, ChunkPool, DRAWS_ARRAY_CAPACITY, LayerPipeline, PlainLayer,
        RawLayer, RawLayerChunk, Standalone, WriteMode,
        brush::{
            blur::BlurBrush,
            param::{BrushParam, BrushParamKey, BrushValue, BrushValueMut},
            pixel::PixelBrush,
            round::RoundBrush,
            smudge::SmudgeBrush,
            tint::TintBrush,
        },
        chunks_key_to_rect, rect_chunks_iter, rect_chunks_stack_iter,
        stream::ThreadInput,
        write_dispatch,
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

    pub fn batch<T: Brush + ?Sized>(&self, brush: &T, target: Draw) -> Option<DrawsBatch> {
        // pre-process
        let target = Draw {
            position: target.position,
            force: target.force.clamp(0.0, 1.0),
        };

        let mut draws = Vec::new();
        let (next, dirty, draws_cnt) = brush.processor(self.prev, target, &mut draws);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            None
        } else {
            Some(DrawsBatch {
                prev: self.prev,
                next,
                draws_cnt,
                draws,
                dirty,
                replace: self.replace,
                bridge: self.bridge,
            })
        }
    }

    /// Confirm that the generated batch will be used, and stroke will respond to related dirty
    /// rectangle changes and previous draw updates, etc.
    pub fn confirm_batch(&mut self, batch: &DrawsBatch) {
        self.prev = Some(batch.next);
        if batch.prev.is_none() {
            self.dirty = batch.dirty;
        } else {
            self.dirty = self.dirty.grow(batch.dirty);
        }
    }

    pub fn upload(&self, lp: &LayerPipeline) {
        write_dispatch(&lp.queue, &lp.work_rect, 0, self.dirty);
    }

    // TODO find a way to request stream thread to load chunks

    pub fn submit(
        &self,
        lp: &LayerPipeline,
        scratch: &mut PlainLayer,
        swap: Option<&mut PlainLayer>,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
        pool: &mut ChunkPool,
    ) {
        if self.prev.is_none() {
            log::warn!("stroke submit without init batch");
            return;
        }

        self.upload(lp);

        let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("stroke_submit"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("stroke_submit"),
            timestamp_writes: None,
        });

        lp.raw_remove_associated(&mut cpass, raw, cache, self.dirty);
        self.merge(lp, &mut cpass, raw, scratch);
        self.cleanup(lp, &mut cpass, scratch, pool);
        if let Some(swap) = swap {
            self.cleanup(lp, &mut cpass, swap, pool);
        }
        // skip cache generation since it is already good
        self.cache_recycle_raw(raw);

        drop(cpass);
        lp.queue.submit([encoder.finish()]);
    }

    pub fn discard(
        &self,
        lp: &LayerPipeline,
        scratch: &mut PlainLayer,
        swap: Option<&mut PlainLayer>,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
        pool: &mut ChunkPool,
    ) {
        if self.prev.is_none() {
            log::warn!("stroke discard without init batch");
            return;
        }

        self.upload(lp);

        let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("stroke_discard"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("stroke_discard"),
            timestamp_writes: None,
        });

        self.cleanup(lp, &mut cpass, scratch, pool);
        if let Some(swap) = swap {
            self.cleanup(lp, &mut cpass, swap, pool);
        }
        lp.raw_remove_associated(&mut cpass, raw, cache, self.dirty);
        self.cache_generate_without_scratch(lp, &mut cpass, raw, cache);
        lp.cache_mipmaps(&mut cpass, cache, self.dirty);
        self.cache_recycle_raw(raw);

        drop(cpass);
        lp.queue.submit([encoder.finish()]);
    }

    /// XXX specificated version, we simply set it associated
    pub fn merge(
        &self,
        _lp: &LayerPipeline,
        _cpass: &mut ComputePass,
        raw: &mut RawLayer,
        scratch: &PlainLayer,
    ) {
        for (&chunk_key, _scratch_chunk) in &scratch.chunks {
            raw.thread_tx
                .send(ThreadInput::MarkRawUnsaved(chunk_key))
                .unwrap();
            raw.swap_chunk(chunk_key, RawLayerChunk::Associated);
        }
    }

    fn cleanup(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        scratch: &mut PlainLayer,
        pool: &mut ChunkPool,
    ) {
        if self.replace {
            for (key, chunk) in scratch.chunks.drain() {
                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                lp.clear_chunk_direct(cpass, &chunk, chunk_rect);
                pool.list.push(chunk);
            }
        } else {
            for (key, chunk) in scratch.chunks.drain() {
                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                lp.clear_chunk(cpass, &chunk, chunk_rect, self.dirty);
                pool.list.push(chunk);
            }
        }
    }

    /// Submit scratch to cache. Raw layer's content will be preserved, but may turn from
    /// associated to filled kind.
    pub fn cache_generate_without_scratch(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
    ) {
        for chunk_key in rect_chunks_iter(self.dirty, cache.chunk_size) {
            let chunk_rect = chunks_key_to_rect(chunk_key, cache.chunk_size);

            let Some(cache_chunk) = cache.chunks.get(&(chunk_key.0, chunk_key.1, 0)) else {
                log::error!("cache not loaded");
                continue;
            };
            let Some(raw_chunk) = raw.chunks.get(&chunk_key) else {
                log::error!("raw not loaded");
                continue;
            };

            cache
                .thread_tx
                .send(ThreadInput::MarkCacheUnsaved((chunk_key.0, chunk_key.1, 0)))
                .unwrap();

            match (cache_chunk, raw_chunk) {
                (CacheLayerChunk::Filled(cache_chunk), RawLayerChunk::Filled(raw_chunk)) => {
                    lp.copy_chunk(
                        cpass,
                        raw_chunk,
                        chunk_rect,
                        cache_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                }
                (
                    CacheLayerChunk::Filled(_),
                    RawLayerChunk::Associated | RawLayerChunk::AssociatedCached(_),
                ) => {
                    log::error!("call `lp.raw_remove_associated` before gen cache 0");
                }
                (CacheLayerChunk::Filled(cache_chunk), RawLayerChunk::Transparent) => {
                    lp.clear_chunk(cpass, cache_chunk, chunk_rect, self.dirty);
                }
                (CacheLayerChunk::Transparent, RawLayerChunk::Filled(raw_chunk)) => {
                    let cache_chunk = lp.create_empty_chunk(
                        cache.chunk_size,
                        chunks_key_to_rect(chunk_key, cache.chunk_size),
                    );
                    lp.copy_chunk(
                        cpass,
                        raw_chunk,
                        chunk_rect,
                        &cache_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                    cache.swap_chunk(
                        (chunk_key.0, chunk_key.1, 0),
                        CacheLayerChunk::Filled(cache_chunk),
                    );
                }
                (
                    CacheLayerChunk::Transparent,
                    RawLayerChunk::Associated | RawLayerChunk::AssociatedCached(_),
                ) => {
                    log::error!("call `lp.raw_remove_associated` before gen cache 0");
                }
                (CacheLayerChunk::Transparent, RawLayerChunk::Transparent) => {}
            }
        }
    }

    /// XXX This is specificated version since this version we dont have overlap
    /// objects yet so we assume it will be associated. Not the case soon later.
    pub fn cache_recycle_raw(&self, raw: &mut RawLayer) {
        for chunk_key in rect_chunks_iter(self.dirty, raw.chunk_size) {
            if let Some(RawLayerChunk::Filled(chunk)) = raw.chunks.get(&chunk_key) {
                raw.swap_chunk(chunk_key, RawLayerChunk::AssociatedCached(chunk.clone()));
            }
        }
    }
}

pub struct DrawsBatch {
    pub prev: Option<Draw>,
    pub next: Draw,
    pub draws_cnt: u32,
    pub draws: Vec<u8>,
    pub dirty: Rectangle,
    pub replace: bool,
    pub bridge: bool,
}

impl DrawsBatch {
    pub fn draw<T: Brush + ?Sized>(
        &self,
        lp: &LayerPipeline,
        scratch: &mut PlainLayer,
        swap: &mut Option<PlainLayer>,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
        bridge: &mut Standalone,
        brush: &T,
    ) {
        self.upload(lp, bridge);

        let mut encoder = (lp.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("batch_draw"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("batch_draw"),
            timestamp_writes: None,
        });

        self.draw_prepare_scratch(lp, &mut cpass, raw, cache, scratch);
        if let Some(swap) = swap {
            self.draw_prepare_scratch(lp, &mut cpass, raw, cache, swap);
        }
        self.draw_into_scratch(lp, &mut cpass, scratch, swap, bridge, brush);

        lp.raw_remove_associated(&mut cpass, raw, cache, self.dirty);
        self.cache_generate_with_scratch(lp, &mut cpass, scratch, raw, cache);
        lp.cache_mipmaps(&mut cpass, cache, self.dirty);
        // raw recycle skipped

        drop(cpass);
        lp.queue.submit([encoder.finish()]);
    }

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
        write_dispatch(&lp.queue, &lp.work_rect, 0, self.dirty);
        write_dispatch(&lp.queue, &lp.draws_rect, 0, self.dirty);
        (lp.queue).write_buffer(&lp.draws_length, 0, bytes_of(&self.draws_cnt));
        (lp.queue).write_buffer(&lp.draws_array, 0, &self.draws);
    }

    pub fn satisfied(&self, raw: &RawLayer, cache: &CacheLayers) -> bool {
        for key in rect_chunks_iter(self.dirty, raw.chunk_size) {
            if !raw.chunks.contains_key(&key) {
                return false;
            }
        }

        for key in rect_chunks_stack_iter(self.dirty, cache.chunk_size, cache.mipmap_cap) {
            if !cache.chunks.contains_key(&key) {
                return false;
            }
        }

        true
    }

    pub fn draw_prepare_scratch(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        raw: &RawLayer,
        cache: &CacheLayers,
        scratch: &mut PlainLayer,
    ) {
        for key in rect_chunks_iter(self.dirty, scratch.chunk_size) {
            if scratch.chunks.contains_key(&key) {
                continue;
            }

            if self.replace {
                let raw_chunk = match raw.chunks.get(&key) {
                    Some(RawLayerChunk::Transparent) => None,
                    Some(RawLayerChunk::Filled(chunk)) => Some(chunk),
                    Some(RawLayerChunk::Associated) | Some(RawLayerChunk::AssociatedCached(_)) => {
                        match cache.chunks.get(&(key.0, key.1, 0)) {
                            Some(CacheLayerChunk::Transparent) => None,
                            Some(CacheLayerChunk::Filled(chunk)) => Some(chunk),
                            None => continue,
                        }
                    }
                    None => continue,
                };

                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                let scratch_chunk = lp.create_empty_chunk(scratch.chunk_size, chunk_rect);
                if let Some(raw_chunk) = raw_chunk {
                    lp.copy_chunk_direct(cpass, raw_chunk, chunk_rect, &scratch_chunk, chunk_rect);
                }

                scratch.chunks.insert(key, scratch_chunk);
            } else {
                let chunk_rect = chunks_key_to_rect(key, scratch.chunk_size);
                let scratch_chunk = lp.create_empty_chunk(scratch.chunk_size, chunk_rect);
                scratch.chunks.insert(key, scratch_chunk);
            }
        }
    }

    /// must `upload` before draw
    pub fn draw_into_scratch<T: Brush + ?Sized>(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        scratch: &PlainLayer,
        swap: &Option<PlainLayer>,
        bridge: &Standalone,
        brush: &T,
    ) {
        if self.prev.is_none() {
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
                let swap_chunk = swap.as_ref().and_then(|x| x.chunks.get(&key));
                let Some(chunk) = scratch.chunks.get(&key) else {
                    continue;
                };

                brush.prepare_draw(cpass, lp, &chunk.read);
                lp.draw_chunk(
                    cpass,
                    chunk,
                    chunk_rect,
                    swap_chunk,
                    Some(chunk_rect),
                    brush,
                    self.dirty,
                );
            }
        }
    }

    /// must `upload` before draw
    pub fn draw_into_standalone<T: Brush + ?Sized>(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        canvas: &Standalone,
        swap: &Option<Standalone>,
        brush: &T,
    ) {
        if self.prev.is_none() {
            brush.prepare_stroke(cpass, lp);
        }

        brush.prepare_draw(cpass, lp, &canvas.chunk.read);
        lp.draw_chunk(
            cpass,
            &canvas.chunk,
            canvas.rect,
            swap.as_ref().map(|x| &x.chunk),
            swap.as_ref().map(|x| x.rect),
            brush,
            self.dirty,
        );
    }

    /// Submit scratch to cache. Raw layer's content will be preserved, but may turn from
    /// associated to filled kind.
    pub fn cache_generate_with_scratch(
        &self,
        lp: &LayerPipeline,
        cpass: &mut ComputePass,
        scratch: &PlainLayer,
        raw: &mut RawLayer,
        cache: &mut CacheLayers,
    ) {
        debug_assert_eq!(raw.chunk_size, scratch.chunk_size);

        for (&chunk_key, scratch_chunk) in &scratch.chunks {
            let chunk_rect = chunks_key_to_rect(chunk_key, scratch.chunk_size);

            let Some(cache_chunk) = cache.chunks.get(&(chunk_key.0, chunk_key.1, 0)) else {
                log::error!("cache not loaded");
                continue;
            };
            let Some(raw_chunk) = raw.chunks.get(&chunk_key) else {
                log::error!("raw not loaded");
                continue;
            };

            cache
                .thread_tx
                .send(ThreadInput::MarkCacheUnsaved((chunk_key.0, chunk_key.1, 0)))
                .unwrap();

            match (cache_chunk, raw_chunk) {
                (CacheLayerChunk::Filled(cache_chunk), RawLayerChunk::Filled(raw_chunk)) => {
                    if self.replace {
                        lp.copy_chunk(
                            cpass,
                            scratch_chunk,
                            chunk_rect,
                            &cache_chunk,
                            chunk_rect,
                            self.dirty,
                        );
                    } else {
                        lp.merge_chunk_over_cross(
                            cpass,
                            scratch_chunk,
                            chunk_rect,
                            raw_chunk,
                            chunk_rect,
                            cache_chunk,
                            chunk_rect,
                            self.dirty,
                        );
                    };
                }
                (
                    CacheLayerChunk::Filled(_),
                    RawLayerChunk::Associated | RawLayerChunk::AssociatedCached(_),
                ) => {
                    log::error!("call `lp.raw_remove_associated` before gen cache 0");
                }
                (CacheLayerChunk::Filled(cache_chunk), RawLayerChunk::Transparent) => {
                    lp.copy_chunk(
                        cpass,
                        scratch_chunk,
                        chunk_rect,
                        cache_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                }
                (CacheLayerChunk::Transparent, RawLayerChunk::Filled(raw_chunk)) => {
                    let cache_chunk = lp.create_empty_chunk(
                        cache.chunk_size,
                        chunks_key_to_rect(chunk_key, cache.chunk_size),
                    );
                    if self.replace {
                        lp.copy_chunk(
                            cpass,
                            scratch_chunk,
                            chunk_rect,
                            &cache_chunk,
                            chunk_rect,
                            self.dirty,
                        );
                    } else {
                        lp.merge_chunk_over_cross(
                            cpass,
                            scratch_chunk,
                            chunk_rect,
                            raw_chunk,
                            chunk_rect,
                            &cache_chunk,
                            chunk_rect,
                            self.dirty,
                        );
                    };
                    cache.swap_chunk(
                        (chunk_key.0, chunk_key.1, 0),
                        CacheLayerChunk::Filled(cache_chunk),
                    );
                }
                (
                    CacheLayerChunk::Transparent,
                    RawLayerChunk::Associated | RawLayerChunk::AssociatedCached(_),
                ) => {
                    log::error!("call `lp.raw_remove_associated` before gen cache 0");
                }
                (CacheLayerChunk::Transparent, RawLayerChunk::Transparent) => {
                    let cache_chunk = lp.create_empty_chunk(
                        cache.chunk_size,
                        chunks_key_to_rect(chunk_key, cache.chunk_size),
                    );
                    lp.copy_chunk(
                        cpass,
                        scratch_chunk,
                        chunk_rect,
                        &cache_chunk,
                        chunk_rect,
                        self.dirty,
                    );
                    cache.swap_chunk(
                        (chunk_key.0, chunk_key.1, 0),
                        CacheLayerChunk::Filled(cache_chunk),
                    );
                }
            }
        }
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

    fn set_pipeline(&self, cpass: &mut ComputePass, pipeline: &LayerPipeline, write: WriteMode);
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
