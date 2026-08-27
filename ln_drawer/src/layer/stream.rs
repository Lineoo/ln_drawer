use std::{
    error::Error,
    sync::{
        Arc,
        mpsc::{Receiver, RecvError, Sender, TryRecvError},
    },
    time::Instant,
};

use glam::{I64Vec2, IVec2, UVec2};
use hashbrown::HashSet;
use indexmap::{IndexMap, IndexSet};
use redb::{ReadableDatabase, TableDefinition};
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Extent3d, MapMode, Origin3d,
    PollType, Queue, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase,
    Texture, TextureAspect,
};
use winit::window::Window;

use crate::{
    layer::{Chunk, ChunkKey, LayerPipeline, chunk_to_rect},
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    save::SaveDatabase,
};

const CHUNK_REAL_CAPS: usize = 512;
const CHUNK_HARD_CAPS: usize = 1024;
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
    #[expect(unused)]
    SwapChunk(ChunkKey, Chunk),
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
    /// deprecated
    _mipmapped: bool,
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
    pub chunk_size: u32,
    pub mipmap_levels: u8,
    pub layer_pipeline: Arc<LayerPipeline>,
    pub window: Arc<dyn Window>,
}

/// Static database of real textures that are loaded.
pub struct StreamBase {
    active: IndexMap<ChunkKey, Option<Texture>>,
    unsaved: HashSet<ChunkKey>,
    real_cnt: usize,
}

/// Single batch of streaming chunks selected out of [`StreamQueue`] and waited to
/// be actually loaded. It serves as a cache buffer and should stay clean whenever
/// the thread loop starts.
pub struct StreamStaging {
    active: Vec<ChunkKey>,
}

/// ALL chunks waited to stage _(no matter loaded or not)_, will IMMEDIATELY
/// refresh after camera movement.
pub struct StreamQueue {
    inner: IndexSet<ChunkKey>,
    front: usize,
}

/// Camera information from main thread, used to determine which chunks to load.
pub struct CameraInfo {
    center: ChunkKey,
    rect: Rectangle,
    range: ((i32, i32), (i32, i32)),
    outdated: bool,
}

/// Debug information for diagnosis.
#[derive(Default)]
pub struct DebugInfo {
    decode: usize,
    load: usize,
    load_real: usize,
    encode: usize,
    unload: usize,
    unload_real: usize,
    clean: usize,
}

pub fn loading_thread(
    config: StreamConfig,
    input_rx: Receiver<ThreadInput>,
    output_tx: Sender<ThreadOutput>,
) -> Result<(), Box<dyn Error>> {
    let mut base = StreamBase {
        active: IndexMap::<ChunkKey, Option<Texture>>::new(),
        unsaved: HashSet::new(),
        real_cnt: 0,
    };

    let mut staging = StreamStaging { active: Vec::new() };

    let mut queue = StreamQueue {
        front: 0,
        inner: IndexSet::with_capacity(400),
    };

    let mut camera = CameraInfo {
        center: (0, 0, 0),
        rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(50)),
        range: super::rect_to_chunks(Rectangle::default(), 0, config.chunk_size),
        outdated: false,
    };

    let mut debug = DebugInfo::default();

    loop {
        let input = if queue.front < queue.inner.len() || camera.outdated {
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
                camera.rect = Camera::manual_view_rect(zoom, size, center);
                let stream_center_new = chunk_of(center.q32_round(), zoom, config.chunk_size);
                let stream_range_new =
                    super::rect_to_chunks(camera.rect, stream_center_new.2, config.chunk_size);
                if stream_range_new != camera.range || stream_center_new != camera.center {
                    camera.range = stream_range_new;
                    camera.center = stream_center_new;
                    camera.outdated = true;
                }
                continue;
            }
            Some(ThreadInput::MarkUnsaved(chunk)) => {
                base.unsaved.insert(chunk);
                continue;
            }
            Some(ThreadInput::RequestReal(key)) => {
                if base.active.get(&key).is_some_and(|x| x.is_none()) {
                    let (texture, chunk) = chunk_prepare(&config, key)?;
                    base.active.insert(key, Some(texture));
                    base.unsaved.insert(key);
                    base.real_cnt += 1;
                    debug.load += 1;
                    debug.load_real += 1;
                    output_tx.send(ThreadOutput::Insert(key, chunk))?;
                }
            }
            Some(ThreadInput::SwapChunk(key, chunk)) => {
                base.active.insert(key, Some(chunk.texture.clone()));
                base.unsaved.insert(key);
                base.real_cnt += 1;
                debug.load += 1;
                debug.load_real += 1;
                output_tx.send(ThreadOutput::Insert(key, chunk))?;
            }
            Some(ThreadInput::Autosave) => {
                autosave(&config, &mut base, &mut debug)?;
                continue;
            }
            Some(ThreadInput::Abort) => {
                return Ok(());
            }
            None => {}
        };

        output_tx.send(ThreadOutput::ThreadDebugMessage(format!(
            "Loading Queue: length {} - pending {} \n\
            Texture Index: real {} total {} \n\
            Debug Counter: \n    \
                | load decode {} real {} total {} | \n    \
                | unload encode {} real {} total {} clean {} | \n\
            Camera Center: {:?} \n\
            ",
            queue.inner.len(),
            queue.inner.len() - queue.front,
            base.real_cnt,
            base.active.len(),
            debug.decode,
            debug.load_real,
            debug.load,
            debug.encode,
            debug.unload_real,
            debug.unload,
            debug.clean,
            camera.center
        )))?;

        if camera.outdated {
            restock_queue(&config, &mut camera, &mut queue);
        }

        debug_assert!(staging.active.is_empty(), "staging buffer is not cleared");

        queue_staging(&mut base, &mut staging, &mut queue);

        if staging.active.is_empty() {
            continue;
        }

        base.active
            .sort_by_key(|&key, _| chunk_distance(key, camera.center, config.mipmap_levels));

        unload(
            &config,
            &output_tx,
            &mut base,
            &mut staging,
            &queue,
            &mut debug,
        )?;

        load(&config, &output_tx, &mut base, &mut staging, &mut debug)?;

        config.window.request_redraw();
    }
}

/// camera is moved so we restock the stream queue
fn restock_queue(config: &StreamConfig, camera: &mut CameraInfo, queue: &mut StreamQueue) {
    camera.outdated = false;
    queue.front = 0;
    queue.inner.clear();

    for z in camera.center.2.saturating_sub(1)..config.mipmap_levels {
        let (range_src, range_dst) = super::rect_to_chunks(camera.rect, z, config.chunk_size);
        for x in range_src.0..range_dst.0 {
            for y in range_src.1..range_dst.1 {
                queue.inner.insert((x, y, z));
            }
        }
    }

    debug_assert!(queue.inner.len() < CHUNK_REAL_CAPS - 1);

    queue
        .inner
        .sort_by_key(|&key| chunk_distance(key, camera.center, config.mipmap_levels));
}

/// select certain amount of chunks defined in [`CHUNK_BATCH`] from queue to load in single batch,
fn queue_staging(base: &mut StreamBase, staging: &mut StreamStaging, queue: &mut StreamQueue) {
    let mut batch_cnt = 0;
    while let Some(&key) = queue.inner.get_index(queue.front)
        && batch_cnt < CHUNK_BATCH
    {
        queue.front += 1;
        if base.active.contains_key(&key) {
            continue;
        }

        staging.active.push(key);
        batch_cnt += 1;
    }
}

fn load(
    config: &StreamConfig,
    output_tx: &Sender<ThreadOutput>,
    base: &mut StreamBase,
    staging: &mut StreamStaging,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let read = config.database.0.begin_read()?;
    let table_chunk = read.open_table(TABLE_LAYER_CHUNK)?;
    let table_meta = read.open_table(TABLE_LAYER_CHUNK_META)?;
    for key in staging.active.drain(..) {
        if let Some(data) = table_chunk.get((0, key))? {
            let mut bytes = zstd::decode_all(data.value())?;
            let (texture, chunk) = chunk_prepare(config, key)?;
            debug.decode += 1;

            if let Some(meta) = table_meta.get(((0, key), 0))?
                && let Ok(meta0) = postcard::from_bytes::<ChunkMeta0>(meta.value())
            {
                if meta0.format > CHUNK_META0_FORMAT {
                    log::error!(
                        "Cannot read layer chunk {key:?} from newer version {:?}",
                        meta0.format
                    );
                    continue;
                } else if meta0.format < CHUNK_META0_FORMAT {
                    chunk_migration(&mut bytes, key, &meta0)?;
                    touch_chunk_meta(config, key, meta0)?;
                }
            } else {
                // Edge cases: format 0 for older version that did not add meta0
                let meta0 = ChunkMeta0 {
                    format: 0,
                    _mipmapped: false,
                };

                log::warn!("failed to get metadata from chunk {key:?}",);
                chunk_migration(&mut bytes, key, &meta0)?;
                touch_chunk_meta(config, key, meta0)?;
            }

            chunk_write(config, &bytes, &texture);
            base.active.insert(key, Some(texture));
            base.real_cnt += 1;
            debug.load_real += 1;
            output_tx.send(ThreadOutput::Insert(key, chunk))?;
        } else {
            base.active.insert(key, None);
        }

        debug.load += 1;
    }

    Ok(())
}

fn unload(
    config: &StreamConfig,
    output_tx: &Sender<ThreadOutput>,
    base: &mut StreamBase,
    staging: &mut StreamStaging,
    queue: &StreamQueue,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let write = config.database.0.begin_write()?;
    let mut table_chunk = write.open_table(TABLE_LAYER_CHUNK)?;
    let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;
    let mut frnt = base.active.len();
    while base.real_cnt + staging.active.len() >= CHUNK_REAL_CAPS
        || base.active.len() + staging.active.len() >= CHUNK_HARD_CAPS
    {
        frnt -= 1;
        if (queue.inner).contains(base.active.get_index(frnt).unwrap().0) {
            continue;
        }

        let (key, texture) = base.active.swap_remove_index(frnt).unwrap();
        output_tx.send(ThreadOutput::Remove(key))?;

        if let Some(texture) = &texture
            && base.unsaved.contains(&key)
        {
            let rx = chunk_readback(texture, &config.device, &config.queue, config.chunk_size);
            config.device.poll(PollType::wait_indefinitely()).unwrap();
            let bytes = rx.recv().unwrap();
            write_chunk_data(base, debug, &mut table_chunk, &mut table_meta, key, bytes).unwrap();
        }

        if texture.is_some() {
            base.real_cnt -= 1;
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
    base: &mut StreamBase,
    debug: &mut DebugInfo,
) -> Result<(), Box<dyn Error + 'static>> {
    let now = Instant::now();

    let write = config.database.0.begin_write()?;

    let mut table_chunk = write.open_table(TABLE_LAYER_CHUNK)?;
    let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;
    let mut tasks = Vec::new();

    for &key in &base.unsaved {
        let Some(Some(texture)) = base.active.get(&key) else {
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
        write_chunk_data(base, debug, &mut table_chunk, &mut table_meta, key, bytes)?;
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

fn write_chunk_data(
    texel: &mut StreamBase,
    debug: &mut DebugInfo,
    table_chunk: &mut redb::Table<(u64, ChunkKey), &[u8]>,
    table_meta: &mut redb::Table<((u64, ChunkKey), u32), &[u8]>,
    key: ChunkKey,
    bytes: Vec<u8>,
) -> Result<(), Box<dyn Error + 'static>> {
    let mut transparent = true;
    for &byte in &bytes {
        if byte != 0x00 {
            transparent = false;
            break;
        }
    }

    if transparent {
        table_chunk.remove((0, key))?;
        table_meta.remove(((0, key), 0))?;
        debug.clean += 1;
    } else {
        let compressed = zstd::encode_all(&bytes[..], 0)?;
        table_chunk.insert((0, key), &compressed[..])?;
        debug.encode += 1;

        let meta0 = ChunkMeta0 {
            format: CHUNK_META0_FORMAT,
            _mipmapped: true,
        };

        let mut meta_bytes = [0u8; 16];
        postcard::to_slice(&meta0, &mut meta_bytes).unwrap();
        table_meta.insert(((0, key), 0), &meta_bytes[..])?;
    }

    texel.unsaved.remove(&key);

    Ok(())
}

fn chunk_prepare(
    config: &StreamConfig,
    key: (i32, i32, u8),
) -> Result<(Texture, Chunk), Box<dyn Error + 'static>> {
    let texture = super::create_chunk_texture(&config.device, config.chunk_size);
    let chunk = super::create_chunk(
        &config.device,
        &config.layer_pipeline.chunk_layout,
        (&texture).clone(),
        chunk_to_rect(key, config.chunk_size),
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
    bytes: &mut [u8],
    key: ChunkKey,
    meta0: &ChunkMeta0,
) -> Result<(), Box<dyn Error + 'static>> {
    for migrate_format in meta0.format..CHUNK_META0_FORMAT {
        match migrate_format {
            0 => {
                fn linear_to_srgb(v: f32) -> f32 {
                    return match v < 0.0031308 {
                        true => 1.055 * v.powf(1.0 / 2.4) - 0.055,
                        false => v * 12.92,
                    };
                }

                let (chunks, _) = bytes.as_chunks_mut();
                for [r, g, b, _] in chunks {
                    *r = (linear_to_srgb(*r as f32 / 255.) * 255.) as u8;
                    *g = (linear_to_srgb(*g as f32 / 255.) * 255.) as u8;
                    *b = (linear_to_srgb(*b as f32 / 255.) * 255.) as u8;
                }

                log::debug!("gamma fix applied on {key:?}");
            }
            _ => unimplemented!("unsupported migration {migrate_format}"),
        }
    }

    Ok(())
}

fn touch_chunk_meta(
    config: &StreamConfig,
    chunk_id: (i32, i32, u8),
    mut meta0: ChunkMeta0,
) -> Result<(), Box<dyn Error + 'static>> {
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
fn chunk_distance((x, y, z): ChunkKey, (cx, cy, cz): ChunkKey, m: u8) -> u64 {
    let dx = (x << z).abs_diff(cx << cz).saturating_sub(1 << z) as u64;
    let dy = (y << z).abs_diff(cy << cz).saturating_sub(1 << z) as u64;
    (dx + dy << m) + (255 - z) as u64
}

fn chunk_of(center: IVec2, zoom: i64, chunk_size: u32) -> ChunkKey {
    let mipmap = (-zoom).q32_round().max(0) as u8;
    let size = (chunk_size << mipmap) as i32;
    (center.x.div_euclid(size), center.y.div_euclid(size), mipmap)
}

#[cfg(test)]
mod test {
    #[test]
    fn chunk_order_overlap() {
        let golden: [super::ChunkKey; _] = [
            (0, 1, 1),
            (0, 0, 1),
            (0, 2, 0),
            (0, 1, 0),
            (1, 1, 0),
            (0, 0, 0),
            (1, 0, 0),
        ];

        let center: super::ChunkKey = (0, 2, 0);
        let mut test = golden.clone();
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center, 8));
        assert_eq!(test, golden);
    }

    #[test]
    fn chunk_order_random() {
        // raw data from script/gen/chunks.py
        let golden: [super::ChunkKey; _] = [
            (0, -1, 8),
            (-1, 0, 8),
            (-1, -1, 8),
            (-1, -1, 7),
            (0, -1, 7),
            (-1, 0, 7),
            (0, 0, 6),
            (-1, -1, 6),
            (0, -1, 5),
            (-3, 2, 1),
            (-13, -3, 0),
            (-1, 2, 4),
            (-1, -2, 5),
            (-14, 5, 1),
            (-10, 0, 2),
            (-6, -41, 0),
            (23, 26, 0),
            (-3, 2, 4),
            (-30, -22, 0),
            (-6, -9, 2),
            (-8, -49, 0),
            (44, 19, 0),
            (7, 3, 3),
            (15, 3, 2),
            (-11, -7, 2),
            (26, 10, 1),
            (-57, 21, 0),
            (-35, 46, 0),
            (6, 6, 3),
            (16, -27, 1),
            (-20, -24, 1),
            (-57, -29, 0),
            (14, -31, 1),
            (-56, -41, 0),
            (-30, 29, 1),
        ];

        let center: super::ChunkKey = (0, 0, 0);
        let mut test = golden.clone();
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center, 8));
        assert_eq!(test, golden);
    }

    #[test]
    fn chunk_order_key() {
        // raw data from script/gen/chunks.py
        let golden: [(super::ChunkKey, u64); _] = [
            ((0, -1, 2), 765),
            ((-1, 3, 1), 766),
            ((1, 2, 1), 1278),
            ((-5, 1, 1), 1278),
            ((-3, -1, 2), 1789),
            ((-4, 4, 1), 1790),
            ((-10, 0, 0), 1791),
            ((-1, -3, 2), 2813),
            ((-5, 5, 1), 2814),
            ((-7, -7, 0), 2815),
            ((-6, -9, 0), 3071),
            ((-15, -1, 0), 3327),
            ((-11, -7, 0), 3839),
            ((2, -3, 2), 4861),
            ((7, 3, 1), 4862),
            ((15, 3, 0), 4863),
            ((4, 7, 1), 5374),
            ((6, 6, 1), 5886),
            ((8, -15, 0), 7167),
            ((7, -16, 0), 7167),
        ];

        let center: super::ChunkKey = (-2, 1, 1);
        let mut test = golden.clone();
        test.iter_mut()
            .for_each(|(chunk, key)| *key = super::chunk_distance(*chunk, center, 8));
        test.sort_by_key(|(_, key)| *key);
        assert_eq!(test, golden);
    }
}
