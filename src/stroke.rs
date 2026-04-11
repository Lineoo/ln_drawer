mod chunk;
mod round_brush;

use hashbrown::{HashMap, HashSet};
use palette::{Srgba, WithAlpha};
use redb::{Database, MultimapTableDefinition, ReadableTable, TableDefinition, WriteTransaction};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ShaderStages,
};
use winit::event::PointerKind;

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, PositionFract, Rectangle, Size},
    render::{
        Render, RenderControl, RenderInformation,
        camera::{Camera, CameraUtils},
    },
    save::{Autosave, SaveDatabase, stream::SaveStream},
    stroke::{
        chunk::{StrokeChunk, StrokeChunkPipeline},
        round_brush::{RoundBrush, RoundBrushPipeline, RoundBrushStorage},
    },
    tools::{
        collider::ToolCollider,
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
    world::{Element, Handle, World},
};

const CHUNK_SIZE: u32 = 512;
const CHUNK_CAPS: usize = 5000;
const MAX_STROKE: u64 = 1000;

const TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
    MultimapTableDefinition::new("stroke");
const TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> = TableDefinition::new("stroke_chunk");

pub struct StrokeLayer {
    pub chunks: HashMap<(i32, i32), Option<Handle<StrokeChunk>>>,
    unsaved: HashSet<(i32, i32)>,
    stream: SaveStream<(i32, i32)>,

    pub front_color: Srgba,
    pub brush: RoundBrush,
    pub modifier: BrushModifier,

    current: Option<BrushInstance>,

    draw: BindGroup,
    draw_data: Buffer,
}

pub struct BrushModifier {
    pub min_size: f32,
    pub max_size: f32,
    pub size_force_exp: f32,
    pub min_flow: f32,
    pub max_flow: f32,
    pub flow_force_exp: f32,
    pub softness: f32,
    pub step: Option<f32>,
}

#[derive(Clone, Copy)]
struct BrushInstance {
    position: PositionFract,
    force: f32,
    step: Fract,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    dirty_coords: [i32; 2],
    stroke_count: u32,
    _pad: u32,
}

impl Element for StrokeLayer {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        // ensure singleton
        world.single::<StrokeLayer>().unwrap();

        let db = world.single_fetch::<SaveDatabase>().unwrap();
        self.database_init(&db.0).unwrap();

        self.attach_touch(world, this);
        self.attach_autosave(world, this);
        self.attach_render(world, this);
    }
}

impl StrokeLayer {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let draw = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("draw"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let canvas_chunk_pipeline = StrokeChunkPipeline::new(world);
        let round_brush_pipeline =
            RoundBrushPipeline::new(&render, &canvas_chunk_pipeline.compute, &draw);

        let draw_data = device.create_buffer(&BufferDescriptor {
            label: Some("draw_data"),
            size: size_of::<DrawUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draw = device.create_bind_group(&BindGroupDescriptor {
            label: Some("draw"),
            layout: &draw,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &draw_data,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let default_brush = RoundBrush::new(&render, &round_brush_pipeline);
        let default_modifier = BrushModifier {
            min_size: 0.5,
            max_size: 6.0,
            size_force_exp: 1.0,
            min_flow: 0.1,
            max_flow: 1.0,
            flow_force_exp: 2.0,
            softness: 0.5,
            step: None,
        };

        world.insert(canvas_chunk_pipeline);
        world.insert(round_brush_pipeline);

        StrokeLayer {
            chunks: HashMap::new(),
            unsaved: HashSet::new(),
            stream: SaveStream::new(CHUNK_CAPS),
            front_color: palette::named::BLACK.with_alpha(1.0).into_format(),
            brush: default_brush,
            modifier: default_modifier,
            current: None,
            draw,
            draw_data,
        }
    }

    fn database_init(&mut self, db: &Database) -> Result<(), redb::Error> {
        let write = db.begin_write()?;
        write.open_multimap_table(TABLE_STROKE)?;
        write.open_table(TABLE_STROKE_CHUNK)?;
        write.commit()?;
        Ok(())
    }

    fn chunk_autoload(world: &World) -> Option<RenderInformation> {
        let camera = world.single_fetch::<Camera>().unwrap();
        let center = camera.center.round();
        let (chunk_center_x, chunk_center_y) = (
            center.x.div_euclid(CHUNK_SIZE as i32),
            center.y.div_euclid(CHUNK_SIZE as i32),
        );

        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
        let mut buf = Vec::with_capacity(400);
        for chunk_x in chunk_center_x - 10..chunk_center_x + 10 {
            for chunk_y in chunk_center_y - 10..chunk_center_y + 10 {
                buf.push((chunk_x, chunk_y));
            }
        }

        let db = world.single_fetch::<SaveDatabase>().unwrap();
        let write = db.0.begin_write().unwrap();

        let seq = stroke.stream.load_filtered(&buf);
        for (chunk_id, load) in seq {
            if load {
                stroke.try_read_chunk(&write, world, chunk_id).unwrap();
            } else {
                stroke.try_free_chunk(&write, world, chunk_id).unwrap();
            }
        }

        write.commit().unwrap();

        Some(RenderInformation {
            keep_redrawing: false,
        })
    }

    fn try_read_chunk(
        &mut self,
        write: &WriteTransaction,
        world: &World,
        chunk_id: (i32, i32),
    ) -> Result<(), redb::Error> {
        let table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;

        if let Some(chunk) = table_chunk.get(chunk_id)? {
            let bytes = zstd::decode_all(chunk.value()).unwrap();
            let chunk = StrokeChunk::from_bytes(world, chunk_id, &bytes);
            self.chunks.insert(chunk_id, Some(world.insert(chunk)));
        } else {
            self.chunks.insert(chunk_id, None);
        }

        Ok(())
    }

    fn try_free_chunk(
        &mut self,
        write: &WriteTransaction,
        world: &World,
        chunk_id: (i32, i32),
    ) -> Result<(), redb::Error> {
        let Some(chunk_handle) = self.chunks.remove(&chunk_id).unwrap() else {
            return Ok(());
        };

        let mut table = write.open_multimap_table(TABLE_STROKE)?;
        let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;

        if self.unsaved.remove(&chunk_id) {
            let chunk = world.fetch(chunk_handle).unwrap();
            let bytes = chunk.device_readback(world);
            let compressed = zstd::encode_all(&bytes[..], 0).unwrap();
            table.insert((), chunk_id).unwrap();
            table_chunk.insert(chunk_id, &compressed[..]).unwrap();
        }

        world.remove(chunk_handle).unwrap();
        Ok(())
    }

    fn attach_autosave(&mut self, world: &World, this: Handle<Self>) {
        let save = world.insert(Autosave(Box::new(move |world, write| {
            let this = &mut *world.fetch_mut(this).unwrap();
            let mut table = write.open_multimap_table(TABLE_STROKE).unwrap();
            let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK).unwrap();
            for chunk_id in this.unsaved.drain() {
                let Some(&Some(chunk)) = this.chunks.get(&chunk_id) else {
                    continue;
                };

                let chunk = world.fetch(chunk).unwrap();
                let bytes = chunk.device_readback(world);
                let compressed = zstd::encode_all(&bytes[..], 0).unwrap();
                table.insert((), chunk_id).unwrap();
                table_chunk.insert(chunk_id, &compressed[..]).unwrap();
            }
        })));

        world.dependency(save, this);
    }

    fn attach_render(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(Self::chunk_autoload)),
            draw: Some(Box::new(|world, rpass| {
                let stroke = world.single_fetch::<StrokeLayer>().unwrap();
                for &chunk in stroke.chunks.values().flatten() {
                    let chunk = world.fetch(chunk).unwrap();
                    chunk.redraw(world, rpass);
                }
            })),
        });
        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }

    fn attach_touch(&mut self, world: &World, this: Handle<Self>) {
        let collider = world.insert(ToolCollider::fullscreen(-100));
        world.dependency(collider, this);

        let mut pinch_distance = None;
        world.observer(collider, move |event: &MultiTouchGroup, world| {
            let primary = event.members.first().unwrap();

            if matches!(event.active.pointer, PointerKind::Touch(_)) || event.members.len() != 1 {
                let mut sum = [0f64; 2];
                for member in &event.members {
                    sum[0] += member.screen[0];
                    sum[1] += member.screen[1];
                }

                let cnt = event.members.len() as f64;
                let center = [sum[0] / cnt, sum[1] / cnt];

                let mut camera_utils = world.single_fetch_mut::<CameraUtils>().unwrap();

                match event.active.status {
                    MultiTouchStatus::Press => {
                        camera_utils.locked(false);
                        camera_utils.cursor(world, center);
                        camera_utils.anchor_on_screen(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Holding => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(true);
                    }
                    MultiTouchStatus::Release => {
                        camera_utils.cursor(world, center);
                        camera_utils.locked(false);
                    }
                }

                if event.members.len() == 2 {
                    let first = event.members.first().unwrap().screen;
                    let last = event.members.last().unwrap().screen;

                    let (x, y) = (first[0] - last[0], first[1] - last[1]);
                    let cur = (x * x + y * y).sqrt();
                    let prev = pinch_distance.get_or_insert(cur);
                    camera_utils.zoom_delta(world, Fract::from_f64((cur - *prev) * 2.0));
                    *prev = cur;
                } else {
                    pinch_distance = None;
                }
            } else if let MultiTouchStatus::Holding | MultiTouchStatus::Press = primary.status {
                let mut this = world.fetch_mut(this).unwrap();
                let target = BrushInstance {
                    position: primary.position.into_fract(),
                    force: primary.data.force.unwrap_or(1.0),
                    step: Fract::ZERO,
                };

                this.paint(target, world);
            } else {
                world.queue(move |world| {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.current = None;
                });
            }
        });
    }

    fn paint(&mut self, target: BrushInstance, world: &World) {
        // generate draws //

        let previous = *self.current.get_or_insert(target);
        let mut working = previous;
        let mut brushes = Vec::new();
        let mut dirty_box = Rectangle::new_half(working.position.round(), Size::splat(0));

        while working.position.distance(target.position) >= working.step
            && brushes.len() < MAX_STROKE as usize
        {
            working.position = working.position.move_towards(target.position, working.step);

            let previous_distance = previous.position.distance(target.position).into_f32();
            let working_distance = working.position.distance(target.position).into_f32();
            let progress = match working_distance < 1e-6 {
                true => 1.0,
                false => 1.0 - working_distance / previous_distance,
            };

            working.force = (1.0 - progress) * previous.force + progress * target.force;

            // apply brush modifier //

            let modifier = &self.modifier;

            let size = modifier.min_size
                + (modifier.max_size - modifier.min_size)
                    * working.force.powf(modifier.size_force_exp);

            let flow = modifier.min_flow
                + (modifier.max_flow - modifier.min_flow)
                    * working.force.powf(modifier.flow_force_exp);

            let softness = modifier.softness;

            let color = self.front_color;

            brushes.push(RoundBrushStorage {
                color: [color.red, color.green, color.blue, color.alpha],
                position: working.position.round().into_array(),
                force: working.force,
                size,
                softness,
                flow,
                _pad: 0,
            });

            dirty_box = dirty_box.grow(Rectangle::new_half(
                working.position.round(),
                Size::splat((size * 2.0).ceil() as u32),
            ));

            working.step = Fract::from_f32(match self.modifier.step {
                Some(step) => step,
                None => size / 5.0,
            });
        }

        self.current = Some(working);

        let draw = DrawUniform {
            dirty_coords: dirty_box.origin.into_array(),
            stroke_count: brushes.len() as u32,
            _pad: 0,
        };

        self.paint_batch(dirty_box, draw, brushes, world);
    }

    fn paint_batch(
        &mut self,
        dirty_box: Rectangle,
        paint: DrawUniform,
        brushes: Vec<RoundBrushStorage>,
        world: &World,
    ) {
        let chunk_src = (
            dirty_box.left().div_euclid(CHUNK_SIZE as i32),
            dirty_box.down().div_euclid(CHUNK_SIZE as i32),
        );

        let chunk_dst = (
            (dirty_box.right() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
            (dirty_box.up() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
        );

        let mut chunks = Vec::new();
        for chunk_x in chunk_src.0..chunk_dst.0 {
            for chunk_y in chunk_src.1..chunk_dst.1 {
                let chunk_id = (chunk_x, chunk_y);
                if let Some(&chunk) = self.chunks.get(&chunk_id) {
                    chunks.push((chunk_id, chunk));
                }
            }
        }

        for (chunk_id, chunk) in chunks {
            if let Some(chunk) = chunk {
                let mut chunk = world.fetch_mut(chunk).unwrap();
                self.paint_chunk(dirty_box, chunk_id, &mut chunk, &paint, &brushes, world);
            } else {
                let mut chunk = StrokeChunk::new(world, chunk_id);
                self.paint_chunk(dirty_box, chunk_id, &mut chunk, &paint, &brushes, world);
                let ret = self.chunks.insert(chunk_id, Some(world.insert(chunk)));
                debug_assert_eq!(ret, Some(None));
            }
        }
    }

    fn paint_chunk(
        &mut self,
        dirty_box: Rectangle,
        chunk_id: (i32, i32),
        chunk: &mut StrokeChunk,
        paint: &DrawUniform,
        brushes: &[RoundBrushStorage],
        world: &World,
    ) {
        if dirty_box.extend.w == 0 || dirty_box.extend.h == 0 {
            return;
        }

        self.unsaved.insert(chunk_id);

        let brush_pipeline = world.single_fetch::<RoundBrushPipeline>().unwrap();
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        render
            .queue
            .write_buffer(&self.draw_data, 0, bytemuck::bytes_of(paint));

        render.queue.write_buffer(
            &self.brush.brush_data_array,
            0,
            bytemuck::cast_slice(brushes),
        );

        // start compute pass

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("brush"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("brush"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&brush_pipeline.pipeline);
        cpass.set_bind_group(0, Some(&chunk.compute), &[]);
        cpass.set_bind_group(1, Some(&self.draw), &[]);
        cpass.set_bind_group(2, Some(&self.brush.brush), &[]);

        const WORKGROUP_SIZE: Size = Size::new(16, 16);
        cpass.dispatch_workgroups(
            (dirty_box.extend.w - 1) / WORKGROUP_SIZE.w + 1,
            (dirty_box.extend.h - 1) / WORKGROUP_SIZE.h + 1,
            1,
        );

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}
