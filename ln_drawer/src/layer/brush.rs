pub mod blur;
pub mod param;
pub mod round;

use std::{
    mem::size_of,
    sync::{Arc, mpsc::Sender},
};

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{I64Vec2, UVec2};
use hashbrown::HashMap;
use wgpu::{CommandEncoderDescriptor, ComputePass, ComputePassDescriptor, RenderPass};

use crate::{
    layer::{
        Chunk, ChunkPool, DEFAULT_CHUNK_SIZE, DEFAULT_MIPMAP_DISABLED, DRAWS_ARRAY_CAPACITY, Layer,
        LayerPipeline, chunk_to_rect, create_chunk, create_chunk_texture, dispatch_workgroups,
        dispatch_workgroups_extend, rect_to_chunks, stream::ThreadInput, write_dispatch,
    },
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
};

const BRIDGE_CHUNK_SIZE: u32 = 1024;

pub struct DrawPipeline {
    pub layer: Arc<LayerPipeline>,

    // TODO currently unintendedly public for undo/redo system
    pub scratch_dst: Layer,
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
    fn draw(&self, dst: &Layer, pipeline: &mut DrawPipeline, target: Draw);
}

pub trait BrushInner {
    type Draw: Clone + Copy + Pod + Zeroable;

    fn process(&self, draw: Draw) -> Self::Draw;
    fn step(&self, draw: Self::Draw) -> f32;
    fn dirty(&self, draw: Self::Draw) -> Rectangle;

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
    pub fn draw<T: BrushInner>(&mut self, dst: &Layer, brush: &T, target: Draw) {
        let mut draws = Vec::new();

        let prev = self.prev.unwrap_or_else(|| {
            draws.push(brush.process(target));
            target
        });

        let prev_position = prev.position.q32_as_f64();
        let target_position = target.position.q32_as_f64();
        let whole_dist = prev_position.distance(target_position);
        let mut curr = prev;
        let mut curr_position = prev_position;
        while curr_position.distance(target_position) >= brush.step(brush.process(curr)) as f64
            && draws.len() < DRAWS_ARRAY_CAPACITY as usize / size_of::<T::Draw>()
        {
            let step = brush.step(brush.process(curr));
            curr_position = curr_position.move_towards(target_position, step as f64);
            curr.position = I64Vec2::q32_from_f64(curr_position);
            let curr_dist = curr_position.distance(target_position);
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - (curr_dist / whole_dist) as f32,
            };
            curr.force = (1.0 - progress) * prev.force + progress * target.force;
            draws.push(brush.process(curr));
        }

        let mut dirty = Rectangle::new_half(target.position.q32_as_i32(), UVec2::ZERO);
        for &draw in &draws {
            dirty = dirty.grow(brush.dirty(draw));
        }

        self.prev = Some(curr);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

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

        if !self.layer.support_read_write {
            self.draw_upload_swap(dst, brush, dirty, bridge_rect);
        } else {
            self.draw_upload_read_write(dst, brush, dirty, bridge_rect);
        }
    }

    fn draw_upload_swap<T: BrushInner>(
        &mut self,
        dst: &Layer,
        brush: &T,
        dirty: Rectangle,
        bridge_rect: Rectangle,
    ) {
        // prepare

        let mut encoder = (self.layer.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("layer_draw"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_draw"),
            timestamp_writes: None,
        });

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
            &mut cpass,
        );
        self.layer.prepare_chunks(
            &mut self.scratch_swp,
            reference_layer,
            &mut self.scratch_pool,
            dirty,
            &mut cpass,
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
                    dispatch_workgroups(&mut cpass, &[bridge_rect, src_rect]);
                }
            }
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

                    brush.set_pipeline(&mut cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&self.bridge.read), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.write), &[]);
                    dispatch_workgroups(&mut cpass, &[dirty, scratch_rect, bridge_rect]);
                } else {
                    let (Some(dst_chunk), Some(swp_chunk)) = (
                        self.scratch_dst.chunks.get(&key),
                        self.scratch_swp.chunks.get(&key),
                    ) else {
                        continue;
                    };

                    brush.set_pipeline(&mut cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.write), &[]);
                    dispatch_workgroups(&mut cpass, &[dirty, scratch_rect]);

                    cpass.set_pipeline(&self.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&self.layer.dispatch_group), &[0]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
                    dispatch_workgroups(&mut cpass, &[dirty, scratch_rect]);
                };
            }
        }

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);
    }

    fn draw_upload_read_write<T: BrushInner>(
        &mut self,
        dst: &Layer,
        brush: &T,
        dirty: Rectangle,
        bridge_rect: Rectangle,
    ) {
        // prepare

        let mut encoder = (self.layer.device).create_command_encoder(&CommandEncoderDescriptor {
            label: Some("layer_draw"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_draw"),
            timestamp_writes: None,
        });

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
            &mut cpass,
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
                    dispatch_workgroups(&mut cpass, &[bridge_rect, src_rect]);
                }
            }
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

                    brush.set_pipeline(&mut cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&self.bridge.read), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.write), &[]);
                    dispatch_workgroups(&mut cpass, &[dirty, scratch_rect, bridge_rect]);
                } else {
                    let Some(dst_chunk) = self.scratch_dst.chunks.get(&key) else {
                        continue;
                    };

                    brush.set_pipeline(&mut cpass, &self.layer);
                    cpass.set_bind_group(0, Some(&self.layer.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read_write), &[]);
                    cpass.set_bind_group(2, Some(&dst_chunk.read_write), &[]);
                    dispatch_workgroups(&mut cpass, &[dirty, scratch_rect]);
                };
            }
        }

        drop(cpass);
        self.layer.queue.submit([encoder.finish()]);
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
