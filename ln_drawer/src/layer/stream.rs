use std::{
    error::Error,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
    time::Instant,
};

use glam::{I64Vec2, IVec2, UVec2};
use hashbrown::HashSet;
use indexmap::{IndexMap, IndexSet};
use redb::{ReadableDatabase, TableDefinition};
use wgpu::{
    BindGroupLayout, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Extent3d,
    MapMode, Origin3d, PollType, Queue, TexelCopyBufferInfoBase, TexelCopyBufferLayout,
    TexelCopyTextureInfoBase, Texture, TextureAspect,
};

use crate::{
    layer::{Chunk, ChunkKey},
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    save::SaveDatabase,
};

const CHUNK_CAPS: usize = 512;
const CHUNK_BATCH: usize = 8;
const CHUNK_META0_FORMAT: u32 = 1;

const TABLE_LAYER_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
    TableDefinition::new("stroke_chunk");
const TABLE_LAYER_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
    TableDefinition::new("stroke_chunk_meta");

pub enum ThreadInput {
    SetStreamCamera(i64, UVec2, I64Vec2),
    MarkUnsaved(ChunkKey),
    RequestReal(ChunkKey),
    Autosave,
    Abort,
}

pub enum ThreadOutput {
    ThreadDebugMessage(String),
    Insert(ChunkKey, Chunk),
    Remove(ChunkKey),
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct ChunkMeta0 {
    format: u32,
    _mipmapped: bool,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DispatchUniform {
    dispatch_coords: [i32; 2],
    dispatch_size: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    chunk: [i32; 3],
    _pad: u32,
}

pub struct StreamConfig {
    pub database: SaveDatabase,
    pub device: Device,
    pub queue: Queue,
    pub chunk_render_layout: BindGroupLayout,
    pub chunk_draw_layout: BindGroupLayout,
    pub chunk_size: u32,
    pub mipmap_levels: u8,
}

pub struct TexelBase {
    active: IndexMap<ChunkKey, Option<Texture>>,
    staging: IndexSet<ChunkKey>,
    unsaved: HashSet<ChunkKey>,
}

pub struct StreamInfo {
    center: ChunkKey,
    rect: Rectangle,
    range: ((i32, i32), (i32, i32)),
    outdated: bool,
}

pub struct StreamQueue {
    front: usize,
    inner: IndexSet<ChunkKey>,
}

#[derive(Default)]
pub struct DebugInfo {
    load: usize,
    unload: usize,
    load_real: usize,
    unload_real: usize,
    encode: usize,
}

pub fn loading_thread(
    config: StreamConfig,
    input_rx: Receiver<ThreadInput>,
    output_tx: Sender<ThreadOutput>,
) -> Result<(), Box<dyn Error>> {
    let mut texel = TexelBase {
        active: IndexMap::<ChunkKey, Option<Texture>>::new(),
        staging: IndexSet::<ChunkKey>::new(),
        unsaved: HashSet::new(),
    };

    let mut stream = StreamInfo {
        center: (0, 0, 0),
        rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(50)),
        range: super::rect_to_chunks(Rectangle::default(), 0, config.chunk_size),
        outdated: false,
    };

    let mut queue = StreamQueue {
        front: 0,
        inner: IndexSet::with_capacity(400),
    };

    let mut debug = DebugInfo::default();

    loop {
        let input = if queue.front < queue.inner.len() || stream.outdated {
            match input_rx.try_recv() {
                Ok(input) => Some(input),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected)?,
            }
        } else {
            match input_rx.recv() {
                Ok(input) => Some(input),
                Err(RecvError) => Err(RecvError)?,
            }
        };

        match input {
            Some(ThreadInput::SetStreamCamera(zoom, size, center)) => {
                stream.rect = Camera::manual_view_rect(zoom, size, center);
                let stream_center_new = chunk_of(center.q32_round(), zoom, config.chunk_size);
                let stream_range_new =
                    super::rect_to_chunks(stream.rect, stream_center_new.2, config.chunk_size);
                if stream_range_new != stream.range || stream_center_new != stream.center {
                    stream.range = stream_range_new;
                    stream.center = stream_center_new;
                    stream.outdated = true;
                }
                continue;
            }
            Some(ThreadInput::MarkUnsaved(chunk)) => {
                texel.unsaved.insert(chunk);
                continue;
            }
            Some(ThreadInput::RequestReal(chunk_id)) => {
                if texel.active.get(&chunk_id).is_some_and(|x| x.is_none()) {
                    let (texture, chunk) = chunk_prepare(&config, chunk_id)?;
                    texel.active.insert(chunk_id, Some(texture));
                    output_tx.send(ThreadOutput::Insert(chunk_id, chunk))?;
                }
                continue;
            }
            Some(ThreadInput::Autosave) => {
                autosave(&config, &mut texel, &mut debug)?;
                continue;
            }
            Some(ThreadInput::Abort) => {
                return Ok(());
            }
            None => {}
        };

        output_tx.send(ThreadOutput::ThreadDebugMessage(format!(
            "Loading Queue: length {} - pending {} \n\
            Texture Index: real {} / total {} \n\
            Debug Counter: \n    \
                | load {} | load_read {} | \n    \
                | unload {} | unload_real {} | \n    \
                | encode {} | \n\
            Camera Center: {:?} \n\
            ",
            queue.inner.len(),
            queue.inner.len() - queue.front,
            texel.active.values().flatten().count(),
            texel.active.len(),
            debug.load,
            debug.load_real,
            debug.unload,
            debug.unload_real,
            debug.encode,
            stream.center
        )))?;

        if stream.outdated {
            restock_queue(&config, &mut stream, &mut queue);
        }

        staging_queue(&mut texel, &mut queue);

        if texel.staging.is_empty() {
            continue;
        }

        texel
            .active
            .sort_by_key(|&key, _| chunk_distance(key, stream.center, config.mipmap_levels));

        unload(&config, &output_tx, &mut texel, &queue, &mut debug)?;
        load(&config, &output_tx, &mut texel, &mut debug)?;
    }
}

fn restock_queue(config: &StreamConfig, stream: &mut StreamInfo, queue: &mut StreamQueue) {
    stream.outdated = false;
    queue.front = 0;
    queue.inner.clear();

    for z in stream.center.2.saturating_sub(1)..config.mipmap_levels {
        let (range_src, range_dst) = super::rect_to_chunks(stream.rect, z, config.chunk_size);
        for x in range_src.0..range_dst.0 {
            for y in range_src.1..range_dst.1 {
                queue.inner.insert((x, y, z));
            }
        }
    }

    debug_assert!(queue.inner.len() < CHUNK_CAPS - 1);

    queue
        .inner
        .sort_by_key(|&key| chunk_distance(key, stream.center, config.mipmap_levels));
}

fn staging_queue(texel: &mut TexelBase, queue: &mut StreamQueue) {
    let mut batch_cnt = 0;
    while let Some(&key) = queue.inner.get_index(queue.front)
        && batch_cnt < CHUNK_BATCH
    {
        queue.front += 1;
        if texel.active.contains_key(&key) {
            continue;
        }

        texel.active.insert(key, None);
        texel.staging.insert(key);
        batch_cnt += 1;
    }
}

fn load(
    config: &StreamConfig,
    output_tx: &Sender<ThreadOutput>,
    texel: &mut TexelBase,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let read = config.database.0.begin_read()?;
    let table_chunk = read.open_table(TABLE_LAYER_CHUNK)?;
    let table_meta = read.open_table(TABLE_LAYER_CHUNK_META)?;
    Ok(for chunk_id in texel.staging.drain(..) {
        if let Some(data) = table_chunk.get((0, chunk_id))?
            && let Some(meta) = table_meta.get(((0, chunk_id), 0))?
            && let Ok(meta0) = postcard::from_bytes::<ChunkMeta0>(meta.value())
        {
            let bytes = zstd::decode_all(data.value())?;
            let (texture, chunk) = chunk_prepare(config, chunk_id)?;
            chunk_write(config, &bytes, &texture);

            texel.active.insert(chunk_id, Some(texture));
            output_tx.send(ThreadOutput::Insert(chunk_id, chunk))?;

            if meta0.format > CHUNK_META0_FORMAT {
                log::error!(
                    "Cannot read layer chunk from newer version {:?}",
                    meta0.format
                );
                texel.active.insert(chunk_id, None);
                continue;
            } else if meta0.format < CHUNK_META0_FORMAT {
                chunk_migration(config, chunk_id, meta0)?;
            }

            debug.load_real += 1;
        } else {
            texel.active.insert(chunk_id, None);
        }

        debug.load += 1;
    })
}

fn unload(
    config: &StreamConfig,
    output_tx: &Sender<ThreadOutput>,
    texel: &mut TexelBase,
    queue: &StreamQueue,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let write = config.database.0.begin_write()?;
    let mut table_chunk = write.open_table(TABLE_LAYER_CHUNK)?;
    let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;
    let mut frnt = texel.active.len();
    while texel.active.len() + texel.staging.len() >= CHUNK_CAPS {
        frnt -= 1;
        if (queue.inner).contains(texel.active.get_index(frnt).unwrap().0) {
            continue;
        }

        let (key, texture) = texel.active.swap_remove_index(frnt).unwrap();
        output_tx.send(ThreadOutput::Remove(key))?;

        if let Some(texture) = &texture
            && texel.unsaved.contains(&key)
        {
            let rx = chunk_readback(texture, &config.device, &config.queue, config.chunk_size);
            config.device.poll(PollType::wait_indefinitely()).unwrap();
            let bytes = rx.recv().unwrap();
            let compressed = zstd::encode_all(&bytes[..], 0)?;
            table_chunk.insert((0, key), &compressed[..])?;

            let meta0 = ChunkMeta0 {
                format: CHUNK_META0_FORMAT,
                _mipmapped: true,
            };
            let mut meta_bytes = [0u8; 16];
            postcard::to_slice(&meta0, &mut meta_bytes).unwrap();
            table_meta.insert(((0, key), 0), &meta_bytes[..])?;

            texel.unsaved.remove(&key);
            debug.encode += 1;
        }

        if texture.is_some() {
            debug.unload_real += 1;
        }

        debug.unload += 1;
    }

    drop(table_chunk);
    drop(table_meta);

    write.commit()?;

    Ok(())
}

fn autosave(
    config: &StreamConfig,
    texel: &mut TexelBase,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let now = Instant::now();

    let write = config.database.0.begin_write()?;

    let mut table_chunk = write.open_table(TABLE_LAYER_CHUNK)?;
    let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;
    let mut tasks = Vec::new();

    for &key in &texel.unsaved {
        let Some(Some(texture)) = texel.active.get(&key) else {
            continue;
        };

        tasks.push((
            key,
            chunk_readback(texture, &config.device, &config.queue, config.chunk_size),
        ));
    }

    config.device.poll(PollType::wait_indefinitely()).unwrap();

    for (key, rx) in tasks {
        let bytes = rx.recv().unwrap();
        let compressed = zstd::encode_all(&bytes[..], 0)?;
        table_chunk.insert((0, key), &compressed[..])?;

        let meta0 = ChunkMeta0 {
            format: CHUNK_META0_FORMAT,
            _mipmapped: true,
        };
        let mut meta_bytes = [0u8; 16];
        postcard::to_slice(&meta0, &mut meta_bytes).unwrap();
        table_meta.insert(((0, key), 0), &meta_bytes[..])?;

        texel.unsaved.remove(&key);
        debug.encode += 1;
    }
    drop(table_chunk);
    drop(table_meta);

    write.commit()?;

    log::info!(
        "Layer stream autosave finished in {:?}",
        Instant::now().duration_since(now)
    );

    Ok(())
}

fn chunk_prepare(
    config: &StreamConfig,
    chunk_id: (i32, i32, u8),
) -> Result<(Texture, Chunk), Box<dyn Error + 'static>> {
    let texture = super::create_chunk_texture(&config.device, config.chunk_size);
    let chunk = super::create_chunk(
        &config.device,
        config.chunk_size,
        &config.chunk_render_layout,
        &config.chunk_draw_layout,
        (&texture).clone(),
        chunk_id,
    );
    Ok((texture, chunk))
}

fn chunk_write(config: &StreamConfig, bytes: &[u8], texture: &Texture) {
    config.queue.write_texture(
        TexelCopyTextureInfoBase {
            texture: texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytes,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(config.chunk_size * 4),
            rows_per_image: Some(config.chunk_size),
        },
        Extent3d {
            width: config.chunk_size,
            height: config.chunk_size,
            depth_or_array_layers: 1,
        },
    );
}

fn chunk_readback(
    texture: &Texture,
    device: &Device,
    queue: &Queue,
    chunk_size: u32,
) -> Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let readback_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("chunk_readback"),
        size: (chunk_size * chunk_size * 4) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("chunk_readback"),
    });

    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfoBase {
            texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfoBase {
            buffer: &readback_buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(chunk_size * 4),
                rows_per_image: Some(chunk_size),
            },
        },
        Extent3d {
            width: chunk_size,
            height: chunk_size,
            depth_or_array_layers: 1,
        },
    );

    let inner = readback_buffer.clone();
    encoder.map_buffer_on_submit(&readback_buffer, MapMode::Read, .., move |ret| {
        ret.unwrap();

        let view = inner.get_mapped_range(..).unwrap();
        tx.send(view.to_vec()).unwrap();
    });

    queue.submit([encoder.finish()]);

    rx
}

fn chunk_migration(
    config: &StreamConfig,
    chunk_id: (i32, i32, u8),
    mut meta0: ChunkMeta0,
) -> Result<(), Box<dyn Error + 'static>> {
    for migrate_format in meta0.format..CHUNK_META0_FORMAT {
        match migrate_format {
            0 => {
                // log::trace!("gamma fixed {chunk_id:?}");
                // gamma_fix.execute(
                //     &config.device,
                //     &config.queue,
                //     &texture,
                //     chunk_id,
                //     config.chunk_size,
                // );
            }
            _ => unimplemented!("unsupported migration {migrate_format}"),
        }
    }
    meta0.format = CHUNK_META0_FORMAT;
    let write = config.database.0.begin_write()?;
    let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;
    let bytes = postcard::to_stdvec(&meta0)?;
    table_meta.insert(((0, chunk_id), 0), &bytes[..])?;
    drop(table_meta);
    write.commit()?;
    Ok(())
}

/// Guaranteed assumption: Upper layer is always loaded first
fn chunk_distance((x, y, z): ChunkKey, (cx, cy, cz): ChunkKey, mipmap: u8) -> u32 {
    fn scale(mipmap: u8) -> i32 {
        2i32.pow(mipmap as u32)
    }

    let dx = (x * scale(z) + scale(z.saturating_sub(1)))
        - (cx * scale(cz) + scale(cz.saturating_sub(1)));
    let dy = (y * scale(z) + scale(z.saturating_sub(1)))
        - (cy * scale(cz) + scale(cz.saturating_sub(1)));
    let dz = (mipmap - z) as i32 * 0x8000;
    dx.unsigned_abs() + dy.unsigned_abs() + dz.unsigned_abs()
}

fn chunk_of(center: IVec2, zoom: i64, chunk_size: u32) -> ChunkKey {
    let mipmap = (-zoom.q32_round()).max(0) as u8;
    let size = chunk_size as i32 * (1i32 << mipmap as i32);
    (center.x.div_euclid(size), center.y.div_euclid(size), mipmap)
}
