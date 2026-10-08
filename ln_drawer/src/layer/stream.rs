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
use redb::{Database, ReadableDatabase};
use smallvec::SmallVec;
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Extent3d, MapMode, Origin3d,
    PollType, Queue, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase,
    Texture, TextureAspect,
};
use winit::window::Window;

use crate::{
    layer::{
        CacheLayerChunk, ChunkKey, LayerPipeline, RawLayerChunk, chunks_key_to_rect,
        chunks_stack_key_to_rect, point_zoom_to_chunks_stack, rect_chunks_stack_layer_iter,
        rect_chunks_stack_layer_range,
    },
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    save::chunk::{ChunkLoad, ChunkSave},
};

const CHUNK_CACHE_REAL_CAPS: usize = 512;
const CHUNK_CACHE_HARD_CAPS: usize = 1024;
const CHUNK_RAW_REAL_CAPS: usize = 64;
const CHUNK_RAW_HARD_CAPS: usize = 512;
const CHUNK_BATCH: usize = 8;

// TODO use Vec<ChunkKey> to minimize thread collision
pub enum ThreadInput {
    SetPage(u64),
    SetStreamCamera(i64, UVec2, I64Vec2),
    MarkRawUnsaved((i32, i32)),
    MarkCacheUnsaved((i32, i32, u8)),
    SwapRaw((i32, i32), RawLayerChunk),
    SwapCache((i32, i32, u8), CacheLayerChunk),
    Autosave,
    Abort,
}

pub enum ThreadOutput {
    ThreadDebugMessage(String),
    RawInsert((i32, i32), RawLayerChunk),
    RawRemove((i32, i32)),
    CacheInsert((i32, i32, u8), CacheLayerChunk),
    CacheRemove((i32, i32, u8)),
}

pub struct StreamConfig {
    pub database: Arc<Database>,
    pub lp: Arc<LayerPipeline>,
    pub window: Arc<dyn Window>,
    pub page: u64,
    pub chunk_size: UVec2,
    pub mipmap_caps: u8,
}

pub struct StreamThread {
    config: StreamConfig,
    input_rx: Receiver<ThreadInput>,
    output_tx: Sender<ThreadOutput>,
    base: ChunksBase,
    queue: LoadQueue,
    camera: CameraInfo,
    debug: DebugInfo,
}

/// Static database of real textures that are loaded.
struct ChunksBase {
    raw: IndexMap<(i32, i32), RawLayerChunk>,
    raw_unsaved: HashSet<(i32, i32)>,
    raw_real_cnt: usize,
    cache: IndexMap<(i32, i32, u8), CacheLayerChunk>,
    cache_unsaved: HashSet<(i32, i32, u8)>,
    cache_real_cnt: usize,
}

/// All chunks waited to stage _(no matter loaded or not)_, will *immediately*
/// refresh after camera movement.
struct LoadQueue {
    outdated: bool,
    raw: IndexSet<(i32, i32)>,
    raw_front: usize,
    cache: IndexSet<(i32, i32, u8)>,
    cache_front: usize,
    /// Single batch of streaming chunks selected out of [`LoadQueue`] and waited to
    /// be actually loaded. It serves as a cache buffer and should stay clean whenever
    /// the thread loop starts.
    batch: SmallVec<[ChunkKeyKind; CHUNK_BATCH]>,
}

enum ChunkKeyKind {
    Raw((i32, i32)),
    Cache((i32, i32, u8)),
}

/// Camera information from main thread, used to determine which chunks to load.
struct CameraInfo {
    center: ChunkKey,
    rect: Rectangle,
    range: ((i32, i32), (i32, i32)),
}

/// Debug information for diagnosis.
#[derive(Default)]
struct DebugInfo {
    decode: usize,
    load: usize,
    load_real: usize,
    encode: usize,
    unload: usize,
    unload_real: usize,
    clean: usize,
}

impl StreamThread {
    pub fn new(
        config: StreamConfig,
        input_rx: Receiver<ThreadInput>,
        output_tx: Sender<ThreadOutput>,
    ) -> Self {
        StreamThread {
            config,
            input_rx,
            output_tx,
            base: ChunksBase {
                raw: IndexMap::new(),
                raw_unsaved: HashSet::new(),
                raw_real_cnt: 0,
                cache: IndexMap::new(),
                cache_unsaved: HashSet::new(),
                cache_real_cnt: 0,
            },
            queue: LoadQueue {
                outdated: false,
                raw: IndexSet::with_capacity(400),
                raw_front: 0,
                cache: IndexSet::with_capacity(400),
                cache_front: 0,
                batch: SmallVec::new(),
            },
            camera: CameraInfo {
                center: (0, 0, 0),
                rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(50)),
                range: ((0, 0), (0, 0)),
            },
            debug: DebugInfo::default(),
        }
    }

    pub fn run(mut self) -> Result<(), Box<dyn Error>> {
        loop {
            match self.recv()? {
                Some(ThreadInput::SetPage(page)) => {
                    self.unload_all()?;
                    self.config.page = page;
                    self.queue.outdated = true;
                }
                Some(ThreadInput::SetStreamCamera(zoom, size, center)) => {
                    self.camera.rect = Camera::view_rect(zoom, size, center);
                    let stream_center_new = point_zoom_to_chunks_stack(
                        center.q32_round(),
                        zoom,
                        self.config.chunk_size,
                    );
                    let stream_range_new = rect_chunks_stack_layer_range(
                        self.camera.rect,
                        self.config.chunk_size,
                        stream_center_new.2,
                    );
                    if stream_range_new != self.camera.range
                        || stream_center_new != self.camera.center
                    {
                        self.camera.range = stream_range_new;
                        self.camera.center = stream_center_new;
                        self.queue.outdated = true;
                    }
                }
                Some(ThreadInput::MarkRawUnsaved(chunk)) => {
                    self.base.raw_unsaved.insert(chunk);
                }
                Some(ThreadInput::MarkCacheUnsaved(chunk)) => {
                    self.base.cache_unsaved.insert(chunk);
                }
                Some(ThreadInput::SwapRaw(key, chunk)) => {
                    // Loading is controlled by stream thread, but the chunk's content is
                    // controlled by the main thread. Since this command replace the chunk
                    // entirely, it is safe to just add chunk into our base (and mark it as
                    // unsaved), without concerning about loading it from db.
                    // NOT sending it back to main thread
                    self.base.raw_insert(key, chunk);
                    self.base.raw_unsaved.insert(key);
                }
                Some(ThreadInput::SwapCache(key, chunk)) => {
                    self.base.cache_insert(key, chunk);
                    self.base.cache_unsaved.insert(key);
                }
                Some(ThreadInput::Autosave) => {
                    self.save_all()?;
                }
                Some(ThreadInput::Abort) => {
                    return Ok(());
                }
                None => {
                    self.send_debug_message()?;

                    if self.queue.outdated {
                        self.queue.restock(&self.config, &self.camera);
                    }

                    self.queue.generate_batch(&mut self.base);
                    if self.queue.batch.is_empty() {
                        continue;
                    }

                    self.unload_overflow()?;
                    self.load()?;

                    self.config.window.request_redraw();
                }
            }
        }
    }

    fn recv(&self) -> Result<Option<ThreadInput>, Box<dyn Error>> {
        let result = if self.queue.still_has_tasks() || self.queue.outdated {
            match self.input_rx.try_recv() {
                Ok(input) => Some(input),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected)?,
            }
        } else {
            match self.input_rx.recv() {
                Ok(input) => Some(input),
                Err(RecvError) => Err(RecvError)?,
            }
        };

        Ok(result)
    }

    fn send_debug_message(&self) -> Result<(), Box<dyn Error>> {
        self.output_tx
            .send(ThreadOutput::ThreadDebugMessage(format!(
                "\
    Loading Queue: length {} - pending {}
    Texture Index: real {} total {}
    Load Counter:
    decode {}
    real {} total {}
    Unload Counter:
    encode {} clean {}
    real {} total {} 
    Camera Center: {:?}",
                self.queue.cache.len(),
                self.queue.cache.len() - self.queue.cache_front,
                self.base.raw_real_cnt,
                self.base.cache.len(),
                self.debug.decode,
                self.debug.load_real,
                self.debug.load,
                self.debug.encode,
                self.debug.clean,
                self.debug.unload_real,
                self.debug.unload,
                self.camera.center
            )))?;
        Ok(())
    }

    fn load(&mut self) -> Result<(), Box<dyn Error>> {
        let read = self.config.database.begin_read()?;
        let read = ChunkLoad::new(&read)?;
        for key in self.queue.batch.drain(..) {
            match key {
                ChunkKeyKind::Cache(key) => {
                    let result = read.load_cache((self.config.page, key))?;
                    if let Some(bytes) = result.texture {
                        let texture = self.config.lp.create_chunk_texture(self.config.chunk_size);
                        let chunk = CacheLayerChunk::Filled(self.config.lp.create_chunk(
                            texture.clone(),
                            chunks_stack_key_to_rect(key, self.config.chunk_size),
                        ));
                        write_texture(&self.config, &bytes, &texture);

                        self.base.cache_insert(key, chunk.clone());
                        self.output_tx.send(ThreadOutput::CacheInsert(key, chunk))?;

                        self.debug.decode += 1;
                        self.debug.load_real += 1;
                    } else {
                        self.base.cache_insert(key, CacheLayerChunk::Transparent);
                        self.output_tx
                            .send(ThreadOutput::CacheInsert(key, CacheLayerChunk::Transparent))?;
                    }
                }
                ChunkKeyKind::Raw(key) => {
                    let result = read.load_raw((self.config.page, key))?;
                    if let Some(bytes) = result.texture {
                        let texture = self.config.lp.create_chunk_texture(self.config.chunk_size);
                        let chunk = RawLayerChunk::Filled(self.config.lp.create_chunk(
                            texture.clone(),
                            chunks_key_to_rect(key, self.config.chunk_size),
                        ));
                        write_texture(&self.config, &bytes, &texture);

                        self.base.raw_insert(key, chunk.clone());
                        self.output_tx.send(ThreadOutput::RawInsert(key, chunk))?;

                        self.debug.decode += 1;
                        self.debug.load_real += 1;
                    } else if result.associated {
                        self.base.raw_insert(key, RawLayerChunk::Associated);
                        self.output_tx
                            .send(ThreadOutput::RawInsert(key, RawLayerChunk::Associated))?;
                    } else {
                        self.base.raw_insert(key, RawLayerChunk::Transparent);
                        self.output_tx
                            .send(ThreadOutput::RawInsert(key, RawLayerChunk::Transparent))?;
                    }
                }
            }

            self.debug.load += 1;
        }

        Ok(())
    }

    fn save_all(&mut self) -> Result<(), Box<dyn Error>> {
        let now = Instant::now();

        let mut write = self.config.database.begin_write()?;
        write.set_quick_repair(true);
        let mut save = ChunkSave::new(&write)?;

        for &key in &self.base.raw_unsaved {
            let Some(chunk) = self.base.raw.get(&key) else {
                continue;
            };

            self.persist_raw(&mut save, key, chunk)?;
        }

        for &key in &self.base.cache_unsaved {
            let Some(chunk) = self.base.cache.get(&key) else {
                continue;
            };

            self.persist_cache(&mut save, key, chunk)?;
        }

        self.config
            .lp
            .device
            .poll(PollType::wait_indefinitely())
            .unwrap();

        drop(save);
        write.commit()?;

        self.base.raw_unsaved.clear();
        self.base.cache_unsaved.clear();

        log::info!(
            "Layer stream autosave finished in {:?}",
            Instant::now().duration_since(now)
        );

        Ok(())
    }

    fn unload_all(&mut self) -> Result<(), Box<dyn Error>> {
        let write = self.config.database.begin_write()?;
        let mut save = ChunkSave::new(&write)?;

        while !self.base.cache.is_empty() {
            self.unload_cache(&mut save, self.base.cache.len() - 1)?;
        }
        while !self.base.raw.is_empty() {
            self.unload_raw(&mut save, self.base.raw.len() - 1)?;
        }

        drop(save);
        write.commit()?;

        Ok(())
    }

    fn unload_overflow(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.base.cache_overflow() && !self.base.raw_overflow() {
            return Ok(());
        }

        self.base.sort(self.camera.center);

        let write = self.config.database.begin_write()?;
        let mut save = ChunkSave::new(&write)?;

        let mut frnt = self.base.cache.len();
        while frnt > 0 && self.base.cache_overflow() {
            frnt -= 1;
            if (self.queue.cache).contains(self.base.cache.get_index(frnt).unwrap().0) {
                continue;
            }

            self.unload_cache(&mut save, frnt)?;
        }

        let mut frnt = self.base.raw.len();
        while frnt > 0 && self.base.raw_overflow() {
            frnt -= 1;
            if (self.queue.raw).contains(self.base.raw.get_index(frnt).unwrap().0) {
                continue;
            }

            self.unload_raw(&mut save, frnt)?;
        }

        drop(save);
        write.commit()?;

        Ok(())
    }

    fn unload_raw(&mut self, save: &mut ChunkSave, index: usize) -> Result<(), Box<dyn Error>> {
        let (key, chunk) = self.base.raw_swap_remove_index(index).unwrap();
        self.output_tx.send(ThreadOutput::RawRemove(key))?;

        if self.base.raw_unsaved.contains(&key) {
            self.persist_raw(save, key, &chunk)?;
            self.base.raw_unsaved.remove(&key);
            self.debug.encode += 1;
            self.debug.unload_real += 1;
        }

        self.debug.unload += 1;

        Ok(())
    }

    fn unload_cache(&mut self, save: &mut ChunkSave, index: usize) -> Result<(), Box<dyn Error>> {
        let (key, chunk) = self.base.cache_swap_remove_index(index).unwrap();
        self.output_tx.send(ThreadOutput::CacheRemove(key))?;

        if self.base.cache_unsaved.contains(&key) {
            self.persist_cache(save, key, &chunk)?;
            self.base.cache_unsaved.remove(&key);
            self.debug.encode += 1;
            self.debug.unload_real += 1;
        }

        self.debug.unload += 1;

        Ok(())
    }

    fn persist_raw(
        &self,
        save: &mut ChunkSave,
        key: (i32, i32),
        chunk: &RawLayerChunk,
    ) -> Result<(), Box<dyn Error>> {
        match chunk {
            RawLayerChunk::Filled(chunk) => {
                let rx = chunk_readback(
                    &chunk.texture,
                    &self.config.lp.device,
                    &self.config.lp.queue,
                    self.config.chunk_size,
                );
                self.config
                    .lp
                    .device
                    .poll(PollType::wait_indefinitely())
                    .unwrap();
                let bytes = rx.recv().unwrap();
                save.save_raw((self.config.page, key), &bytes[..])?;
            }
            RawLayerChunk::Associated => {
                save.save_raw_associated((self.config.page, key))?;
            }
            RawLayerChunk::Transparent => {
                save.save_raw_transparent((self.config.page, key))?;
            }
        }

        Ok(())
    }

    fn persist_cache(
        &self,
        save: &mut ChunkSave,
        key: (i32, i32, u8),
        chunk: &CacheLayerChunk,
    ) -> Result<(), Box<dyn Error>> {
        match chunk {
            CacheLayerChunk::Filled(chunk) => {
                let rx = chunk_readback(
                    &chunk.texture,
                    &self.config.lp.device,
                    &self.config.lp.queue,
                    self.config.chunk_size,
                );
                self.config
                    .lp
                    .device
                    .poll(PollType::wait_indefinitely())
                    .unwrap();
                let bytes = rx.recv().unwrap();
                save.save_cache((self.config.page, key), &bytes[..])?;
            }
            CacheLayerChunk::Transparent => {
                save.save_cache_transparent((self.config.page, key))?;
            }
        }

        Ok(())
    }
}

impl ChunksBase {
    fn sort(&mut self, center: (i32, i32, u8)) {
        self.raw
            .sort_by_key(|&key, _| chunk_distance((key.0, key.1, 0), center));
        self.cache
            .sort_by_key(|&key, _| chunk_distance(key, center));
    }

    fn raw_insert(&mut self, key: (i32, i32), chunk: RawLayerChunk) {
        if matches!(chunk, RawLayerChunk::Filled(_)) {
            self.raw_real_cnt += 1;
        }
        self.raw.insert(key, chunk);
    }

    fn cache_insert(&mut self, key: (i32, i32, u8), chunk: CacheLayerChunk) {
        if matches!(chunk, CacheLayerChunk::Filled(_)) {
            self.cache_real_cnt += 1;
        }
        self.cache.insert(key, chunk);
    }

    fn raw_swap_remove_index(&mut self, index: usize) -> Option<((i32, i32), RawLayerChunk)> {
        let (key, chunk) = self.raw.swap_remove_index(index)?;
        if matches!(chunk, RawLayerChunk::Filled(_)) {
            self.raw_real_cnt -= 1;
        }
        Some((key, chunk))
    }

    fn cache_swap_remove_index(
        &mut self,
        index: usize,
    ) -> Option<((i32, i32, u8), CacheLayerChunk)> {
        let (key, chunk) = self.cache.swap_remove_index(index)?;
        if matches!(chunk, CacheLayerChunk::Filled(_)) {
            self.cache_real_cnt -= 1;
        }
        Some((key, chunk))
    }

    fn cache_overflow(&mut self) -> bool {
        self.cache_real_cnt >= CHUNK_CACHE_REAL_CAPS || self.cache.len() >= CHUNK_CACHE_HARD_CAPS
    }

    fn raw_overflow(&mut self) -> bool {
        self.raw_real_cnt >= CHUNK_RAW_REAL_CAPS || self.raw.len() >= CHUNK_RAW_HARD_CAPS
    }
}

impl LoadQueue {
    /// camera is moved so we restock the stream queue
    fn restock(&mut self, config: &StreamConfig, camera: &CameraInfo) {
        self.outdated = false;
        self.cache_front = 0;
        self.cache.clear();

        for mipmap in camera.center.2.saturating_sub(1)..config.mipmap_caps {
            for key in rect_chunks_stack_layer_iter(camera.rect, config.chunk_size, mipmap) {
                self.cache.insert(key);
            }
        }

        debug_assert!(self.cache.len() < CHUNK_CACHE_REAL_CAPS - 1);

        self.raw
            .sort_by_key(|&key| chunk_distance((key.0, key.1, 0), camera.center));
        self.cache
            .sort_by_key(|&key| chunk_distance(key, camera.center));
    }

    /// select certain amount of chunks defined in [`CHUNK_BATCH`] from queue to load in single batch,
    fn generate_batch(&mut self, base: &mut ChunksBase) {
        debug_assert!(self.batch.is_empty(), "batch is not cleared yet");

        let mut batch_cnt = 0;
        while let Some(&key) = self.cache.get_index(self.cache_front)
            && batch_cnt < CHUNK_BATCH
        {
            self.cache_front += 1;
            if base.cache.contains_key(&key) {
                continue;
            }

            self.batch.push(ChunkKeyKind::Cache(key));
            batch_cnt += 1;
        }
        while let Some(&key) = self.raw.get_index(self.raw_front)
            && batch_cnt < CHUNK_BATCH
        {
            self.raw_front += 1;
            if base.raw.contains_key(&key) {
                continue;
            }

            self.batch.push(ChunkKeyKind::Raw(key));
            batch_cnt += 1;
        }
    }

    fn still_has_tasks(&self) -> bool {
        self.cache_front < self.cache.len()
    }
}

fn write_texture(config: &StreamConfig, bytes: &[u8], texture: &Texture) {
    config.lp.queue.write_texture(
        TexelCopyTextureInfoBase {
            texture: texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        bytes,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(config.chunk_size.x * 4),
            rows_per_image: Some(config.chunk_size.y),
        },
        Extent3d {
            width: config.chunk_size.x,
            height: config.chunk_size.y,
            depth_or_array_layers: 1,
        },
    );
}

fn chunk_readback(
    texture: &Texture,
    device: &Device,
    queue: &Queue,
    chunk_size: UVec2,
) -> Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();

    let readback_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("chunk_readback"),
        size: (chunk_size.x * chunk_size.y * 4) as u64,
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
                bytes_per_row: Some(chunk_size.x * 4),
                rows_per_image: Some(chunk_size.y),
            },
        },
        Extent3d {
            width: chunk_size.x,
            height: chunk_size.y,
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

/// Guaranteed assumption: Upper layer is always loaded first
fn chunk_distance((x, y, z): ChunkKey, (cx, cy, cz): ChunkKey) -> u64 {
    let dx = (x << z).abs_diff(cx << cz).saturating_sub(1 << z) as u64;
    let dy = (y << z).abs_diff(cy << cz).saturating_sub(1 << z) as u64;
    (dx + dy << 8) + (255 - z) as u64
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
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center));
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
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center));
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
            .for_each(|(chunk, key)| *key = super::chunk_distance(*chunk, center));
        test.sort_by_key(|(_, key)| *key);
        assert_eq!(test, golden);
    }
}
