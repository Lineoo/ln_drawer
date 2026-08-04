use bytemuck::{bytes_of, cast_slice};
use glam::{I64Vec2, IVec2, UVec2};
use ln_world::Element;
use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor};

use crate::{
    layer::{
        Layer,
        brush::{Draw, LayerDrawPipeline, Stroke, param::BrushParam},
        dispatch_workgroups, rect_to_chunks, write_dispatch,
    },
    measures::{FI64Ext, Rectangle},
};

#[derive(Clone)]
pub struct BlurBrush {
    pub size: BrushParam<f32>,
    pub sigma: BrushParam<f32>,
    pub softness: BrushParam<f32>,
}

impl BlurBrush {
    pub fn process(&self, draw: Draw) -> BlurDraw {
        BlurDraw {
            position: draw.position,
            softness: self.softness.get(draw),
            size: self.size.get(draw),
            sigma: self.sigma.get(draw),
        }
    }

    pub fn step(&self, draw: BlurDraw) -> f32 {
        draw.size / 5.0
    }

    /// CPU-end draw process
    pub fn draw(&self, pipeline: &mut LayerDrawPipeline, dst: &Layer, target: Draw) {
        let mut draws = Vec::new();
        let next = self.interpolate(pipeline.prev, target, &mut draws);

        let mut dirty = Rectangle::new_half(next.position.q32_as_i32(), UVec2::ZERO);
        for &draw in &draws {
            dirty = dirty.grow(draw.dirty());
        }

        pipeline.prev = Some(next);

        if dirty.extend.x == 0 || dirty.extend.y == 0 {
            return;
        }

        if let Some(stroke) = &mut pipeline.stroke {
            stroke.dirty = stroke.dirty.grow(dirty);
        } else {
            pipeline.stroke = Some(Stroke {
                dirty,
                replace: self.replace_mode(),
                chunks: vec![],
            });
        }

        let mut draws_proc = Vec::with_capacity(draws.len());
        for draw in draws {
            draws_proc.push(draw.into_storage());
        }

        let reference_layer = match self.replace_mode() {
            true => Some(dst),
            false => None,
        };

        // Brush always use uncontrolled layer as scratch
        pipeline.layer.prepare_chunks(
            &mut pipeline.scratch_dst,
            reference_layer,
            &mut pipeline.scratch_pool,
            dirty,
        );
        pipeline.layer.prepare_chunks(
            &mut pipeline.scratch_swp,
            reference_layer,
            &mut pipeline.scratch_pool,
            dirty,
        );

        let queue = &pipeline.layer.queue;
        write_dispatch(queue, &pipeline.draws_dispatch, dirty);
        write_dispatch(queue, &pipeline.layer.dispatch, dirty);

        let draw_length = draws_proc.len() as u32;
        queue.write_buffer(&pipeline.draws_length, 0, bytes_of(&draw_length));
        queue.write_buffer(&pipeline.draws_array, 0, cast_slice(&draws_proc));

        let mut encoder = pipeline
            .layer
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layer_brush"),
            });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("layer_brush"),
            timestamp_writes: None,
        });

        let (start, end) = rect_to_chunks(dirty, 0, pipeline.scratch_dst.chunk_size);
        for x in start.0..end.0 {
            for y in start.1..end.1 {
                let key = (x, y, 0);
                if let Some(dst_chunk) = pipeline.scratch_dst.chunks.get_mut(&key)
                    && let Some(swp_chunk) = pipeline.scratch_swp.chunks.get_mut(&key)
                {
                    if let Some(stroke) = &mut pipeline.stroke {
                        if !stroke.chunks.contains(&key) {
                            stroke.chunks.push(key);
                        }
                    }

                    cpass.set_pipeline(&pipeline.pipelines.blur);
                    cpass.set_bind_group(0, Some(&pipeline.draws_dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.read), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.write), &[]);
                    dispatch_workgroups(&mut cpass, dirty.extend);

                    cpass.set_pipeline(&pipeline.layer.copy_pipeline);
                    cpass.set_bind_group(0, Some(&pipeline.layer.dispatch_group), &[]);
                    cpass.set_bind_group(1, Some(&dst_chunk.write), &[]);
                    cpass.set_bind_group(2, Some(&swp_chunk.read), &[]);
                    dispatch_workgroups(&mut cpass, dirty.extend);
                }
            }
        }

        drop(cpass);
        pipeline.layer.queue.submit([encoder.finish()]);
    }

    pub fn interpolate(&self, prev: Option<Draw>, next: Draw, buf: &mut Vec<BlurDraw>) -> Draw {
        buf.clear();

        let prev = prev.unwrap_or_else(|| {
            buf.push(self.process(next));
            next
        });

        let mut curr_draw = prev;
        let mut curr_proc = self.process(curr_draw);
        let whole_dist = prev
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64());
        while curr_draw
            .position
            .q32_as_f64()
            .distance(next.position.q32_as_f64())
            >= self.step(curr_proc) as f64
            && buf.len() < super::MAX_STROKE as usize
        {
            let step = self.step(curr_proc);
            curr_draw.position = I64Vec2::q32_from_f64(
                curr_draw
                    .position
                    .q32_as_f64()
                    .move_towards(next.position.q32_as_f64(), step as f64),
            );
            let curr_dist = curr_draw
                .position
                .q32_as_f64()
                .distance(next.position.q32_as_f64());
            let progress = match whole_dist < 1e-6 {
                true => 1.0,
                false => 1.0 - (curr_dist / whole_dist) as f32,
            };
            curr_draw.force = (1.0 - progress) * prev.force + progress * next.force;
            curr_proc = self.process(curr_draw);
            buf.push(curr_proc);
        }

        curr_draw
    }

    /// - Normal Mode:
    ///     - Scratch chunks start with __transparent texture__.
    ///     - Render in __over__ mode
    ///     - Merge in __over__ mode
    /// - Replace Mode:
    ///     - Scratch chunks start with __data from destination layer__.
    ///     - Render in __replace__ mode
    ///     - Merge in __replace__ mode
    pub fn replace_mode(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy)]
pub struct BlurDraw {
    pub position: I64Vec2,
    pub softness: f32,
    pub size: f32,
    pub sigma: f32,
}

impl BlurDraw {
    pub fn dirty(self) -> Rectangle {
        Rectangle::new_half(
            self.position.q32_round(),
            UVec2::splat((self.size * 2.0).ceil() as u32),
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurDrawStorage {
    pub position: IVec2,
    pub position_fract: UVec2,
    pub softness: f32,
    pub size: f32,
    pub sigma: f32,
    pub _pad: u32,
}

impl BlurDraw {
    pub fn into_storage(self) -> BlurDrawStorage {
        BlurDrawStorage {
            position: self.position.q32_floor(),
            position_fract: self.position.q32_fract(),
            softness: self.softness,
            size: self.size,
            sigma: self.sigma,
            _pad: 0,
        }
    }
}

impl Element for BlurBrush {}
