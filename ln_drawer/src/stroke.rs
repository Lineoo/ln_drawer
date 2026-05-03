pub mod chunk;
pub mod dirty;
pub mod interpolate;
pub mod modifier;
pub mod shape;

use hashbrown::{HashMap, HashSet};
use ln_world::{Element, Handle, World};
use palette::Srgba;
use redb::{Database, MultimapTableDefinition, ReadableTable, TableDefinition, WriteTransaction};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBinding, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ShaderStages,
};
use winit::event::PointerKind;

use crate::{
    lnwin::Lnwindow,
    measures::{Fract, Rectangle, Size},
    render::{
        Render, RenderControl, RenderInformation,
        camera::{Camera, CameraUtils},
    },
    save::{Autosave, SaveDatabase, stream::SaveStream},
    stroke::{
        chunk::{StrokeChunk, StrokeChunkPipeline},
        dirty::Dirty,
        interpolate::{Draw, Interpolation},
        modifier::{DrawProcessedStorage, Modifier},
        shape::RoundBrush,
    },
    tools::{
        collider::ToolCollider,
        touch::{MultiTouchGroup, MultiTouchStatus},
    },
};

const CHUNK_SIZE: u32 = 512;
const CHUNK_CAPS: usize = 5000;
const MAX_STROKE: u64 = 200;

const TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
    MultimapTableDefinition::new("stroke");
const TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> = TableDefinition::new("stroke_chunk");

const DEFAULT_INTERPOLATION: Interpolation = Interpolation {
    step: |draw| draw.size / 5.0,
};
const DEFAULT_MODIFIER: Modifier = Modifier {
    min_size: 0.5,
    max_size: 6.0,
    size_force_exp: 1.0,
    min_flow: 0.1,
    max_flow: 1.0,
    flow_force_exp: 2.0,
    softness: 0.2,
    color: Srgba::new(0.0, 0.0, 0.0, 1.0),
};
const DEFAULT_DIRTY: Dirty = Dirty {
    bounding: |draw| {
        Rectangle::new_half(
            draw.position.round(),
            Size::splat((draw.size * 2.0).ceil() as u32),
        )
    },
};

pub struct StrokeLayer {
    pub chunks: HashMap<(i32, i32), Option<StrokeChunk>>,
    unsaved: HashSet<(i32, i32)>,
    stream: SaveStream<(i32, i32)>,

    pub interpolation: Interpolation,
    pub modifier: Modifier,
    pub dirty: Dirty,
    pub brush: RoundBrush,
    prev: Option<Draw>,

    dispatch: BindGroup,
    dispatch_meta: Buffer,
    draws_array: Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    dirty_coords: [i32; 2],
    stroke_count: u32,
    _pad: u32,
}

impl StrokeLayer {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let dispatch = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("dispatch"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let canvas_chunk_pipeline = StrokeChunkPipeline::new(world);
        let brush = RoundBrush::new(&render, &dispatch, &canvas_chunk_pipeline.compute);
        world.insert(canvas_chunk_pipeline);

        let dispatch_meta = device.create_buffer(&BufferDescriptor {
            label: Some("dispatch_meta"),
            size: size_of::<DispatchUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let draws_array = device.create_buffer(&BufferDescriptor {
            label: Some("draws_array"),
            size: size_of::<DrawProcessedStorage>() as u64 * MAX_STROKE,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dispatch = device.create_bind_group(&BindGroupDescriptor {
            label: Some("dispatch"),
            layout: &dispatch,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &dispatch_meta,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &draws_array,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        StrokeLayer {
            chunks: HashMap::new(),
            unsaved: HashSet::new(),
            stream: SaveStream::new(CHUNK_CAPS),
            interpolation: DEFAULT_INTERPOLATION,
            modifier: DEFAULT_MODIFIER,
            dirty: DEFAULT_DIRTY,
            brush,
            prev: None,
            dispatch,
            dispatch_meta,
            draws_array,
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
            self.chunks.insert(chunk_id, Some(chunk));
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
        let Some(chunk) = self.chunks.remove(&chunk_id).unwrap() else {
            return Ok(());
        };

        let mut table = write.open_multimap_table(TABLE_STROKE)?;
        let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;

        if self.unsaved.remove(&chunk_id) {
            let bytes = chunk.device_readback(world);
            let compressed = zstd::encode_all(&bytes[..], 0).unwrap();
            table.insert((), chunk_id).unwrap();
            table_chunk.insert(chunk_id, &compressed[..]).unwrap();
        }

        Ok(())
    }

    fn attach_autosave(&mut self, world: &World, this: Handle<Self>) {
        let save = world.insert(Autosave(Box::new(move |world, write| {
            let this = &mut *world.fetch_mut(this).unwrap();
            let mut table = write.open_multimap_table(TABLE_STROKE).unwrap();
            let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK).unwrap();
            for chunk_id in this.unsaved.drain() {
                let Some(Some(chunk)) = this.chunks.get(&chunk_id) else {
                    continue;
                };

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
                let camera = world.single_fetch::<Camera>().unwrap();

                let view_rect = camera.world_view_rect();
                let chunk_src = (
                    view_rect.left().div_euclid(CHUNK_SIZE as i32),
                    view_rect.down().div_euclid(CHUNK_SIZE as i32),
                );
                let chunk_dst = (
                    (view_rect.right() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
                    (view_rect.up() - 1).div_euclid(CHUNK_SIZE as i32) + 1,
                );

                for chunk_x in chunk_src.0..chunk_dst.0 {
                    for chunk_y in chunk_src.1..chunk_dst.1 {
                        if let Some(Some(chunk)) = stroke.chunks.get(&(chunk_x, chunk_y)) {
                            chunk.redraw(world, rpass);
                        }
                    }
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
                let target = Draw {
                    position: primary.position,
                    force: primary.data.force.unwrap_or(1.0),
                };

                this.paint(target, world);
            } else {
                world.queue(move |world| {
                    let mut this = world.fetch_mut(this).unwrap();
                    this.prev = None;
                });
            }
        });
    }

    fn paint(&mut self, next: Draw, world: &World) {
        // generate draws //

        let mut draw_buf = Vec::new();
        let curr = self
            .interpolation
            .interpolate(self.prev, next, &self.modifier, &mut draw_buf);
        self.prev = Some(curr);

        let dirty = self.dirty.compute(curr.position.round(), &draw_buf);
        if dirty.bounding.extend.w == 0 || dirty.bounding.extend.h == 0 {
            return;
        }

        // prepare chunks

        let mut chunks = Vec::new();
        for chunk_x in dirty.chunk_src.0..dirty.chunk_dst.0 {
            for chunk_y in dirty.chunk_src.1..dirty.chunk_dst.1 {
                let chunk_id = (chunk_x, chunk_y);
                if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
                    chunk.get_or_insert_with(|| StrokeChunk::new(world, chunk_id));
                    chunks.push(chunk_id);
                    self.unsaved.insert(chunk_id);
                }
            }
        }

        // assign works to GPU

        let dispatch = DispatchUniform {
            dirty_coords: dirty.bounding.origin.into_array(),
            stroke_count: draw_buf.len() as u32,
            _pad: 0,
        };

        let render = world.single_fetch::<Render>().unwrap();
        let queue = &render.queue;
        let device = &render.device;

        let mut draw_stg = Vec::with_capacity(draw_buf.len());
        for draw in draw_buf {
            draw_stg.push(draw.into_storage());
        }

        queue.write_buffer(&self.dispatch_meta, 0, bytemuck::bytes_of(&dispatch));
        queue.write_buffer(&self.draws_array, 0, bytemuck::cast_slice(&draw_stg));

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("stroke"),
        });

        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("stroke"),
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.brush.pipeline);
        cpass.set_bind_group(0, Some(&self.dispatch), &[]);

        for chunk in chunks {
            let chunk = self.chunks.get(&chunk).unwrap().as_ref().unwrap();

            cpass.set_bind_group(1, Some(&chunk.compute), &[]);

            const WORKGROUP_SIZE: Size = Size::new(16, 16);
            cpass.dispatch_workgroups(
                (dirty.bounding.extend.w - 1) / WORKGROUP_SIZE.w + 1,
                (dirty.bounding.extend.h - 1) / WORKGROUP_SIZE.h + 1,
                1,
            );
        }

        drop(cpass);

        let command = encoder.finish();
        render.queue.submit([command]);

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
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
