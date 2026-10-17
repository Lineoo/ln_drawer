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
        CacheLayerChunk, DirtyChunkPool, LayerPipeline, RawLayerChunk, chunks_key_to_rect,
        chunks_stack_key_to_rect, point_zoom_to_chunks_stack, rect_chunks_iter,
        rect_chunks_stack_layer_iter, rect_chunks_stack_layer_range,
    },
    measures::{FI64Ext, Rectangle},
    render::camera::Camera,
    save::chunk::{ChunkLoad, ChunkLoadResult, ChunkSave},
};

const CHUNK_CACHE_REAL_CAP: usize = 512;
const CHUNK_CACHE_HARD_CAP: usize = 1024;
const CHUNK_RAW_REAL_CAP: usize = 64;
const CHUNK_RAW_HARD_CAP: usize = 512;
const CHUNK_LOAD_BATCH_CAP: usize = 8;
const CHUNK_POOL_CAP: usize = 32;

pub enum ThreadInput {
    SetPage(u64),
    SetStreamCamera(i64, UVec2, I64Vec2),
    #[expect(unused)]
    MarkRawUnsaved(Vec<(i32, i32)>),
    MarkCacheUnsaved(Vec<(i32, i32, u8)>),
    SwapRaw((i32, i32), RawLayerChunk),
    SwapCache((i32, i32, u8), CacheLayerChunk),
    RawRemoveAck(Vec<(i32, i32)>),
    CacheRemoveAck(Vec<(i32, i32, u8)>),
    Autosave,
    Abort,
}

pub enum ThreadOutput {
    ThreadDebugMessage(String),
    RawInsert((i32, i32), RawLayerChunk),
    RawRemoveAsk((i32, i32)),
    CacheInsert((i32, i32, u8), CacheLayerChunk),
    CacheRemoveAsk((i32, i32, u8)),
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
    pool: DirtyChunkPool,
    base: ChunksBase,
    queue: LoadQueue,
    camera: CameraInfo,
}

/// Static database of real textures that are loaded.
struct ChunksBase {
    raw: IndexMap<(i32, i32), RawLayerChunk>,
    raw_unsaved: HashSet<(i32, i32)>,
    raw_real_cnt: usize,
    raw_asked_cnt: usize,
    raw_real_asked_cnt: usize,
    cache: IndexMap<(i32, i32, u8), CacheLayerChunk>,
    cache_unsaved: HashSet<(i32, i32, u8)>,
    cache_real_cnt: usize,
    cache_asked_cnt: usize,
    cache_real_asked_cnt: usize,
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
    batch: SmallVec<[ChunkKeyKind; CHUNK_LOAD_BATCH_CAP]>,
}

enum ChunkKeyKind {
    Raw((i32, i32)),
    Cache((i32, i32, u8)),
}

/// Camera information from main thread, used to determine which chunks to load.
struct CameraInfo {
    center: (i32, i32, u8),
    rect: Rectangle,
    range: ((i32, i32), (i32, i32)),
}

impl StreamThread {
    pub fn new(
        config: StreamConfig,
        input_rx: Receiver<ThreadInput>,
        output_tx: Sender<ThreadOutput>,
    ) -> Self {
        let pool = DirtyChunkPool {
            list: Vec::new(),
            cap: CHUNK_POOL_CAP,
            chunk_size: config.chunk_size,
        };
        StreamThread {
            config,
            input_rx,
            output_tx,
            pool,
            base: ChunksBase {
                raw: IndexMap::new(),
                raw_unsaved: HashSet::new(),
                raw_real_cnt: 0,
                raw_asked_cnt: 0,
                raw_real_asked_cnt: 0,
                cache: IndexMap::new(),
                cache_unsaved: HashSet::new(),
                cache_real_cnt: 0,
                cache_asked_cnt: 0,
                cache_real_asked_cnt: 0,
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
                        zoom + i64::q32_from_f64(0.5),
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
                Some(ThreadInput::MarkRawUnsaved(chunks)) => {
                    for chunk in chunks {
                        self.base.raw_unsaved.insert(chunk);
                    }
                }
                Some(ThreadInput::MarkCacheUnsaved(chunks)) => {
                    for chunk in chunks {
                        self.base.cache_unsaved.insert(chunk);
                    }
                }
                Some(ThreadInput::SwapRaw(key, chunk)) => {
                    // Loading is controlled by stream thread, but the chunk's content is
                    // controlled by the main thread. Since this command replace the chunk
                    // entirely, it is safe to just add chunk into our base (and mark it as
                    // unsaved), without concerning about loading it from db.
                    // NOT sending it back to main thread
                    self.base.raw_insert_recycle(&mut self.pool, key, chunk);
                    self.base.raw_unsaved.insert(key);
                }
                Some(ThreadInput::SwapCache(key, chunk)) => {
                    self.base.cache_insert_recycle(&mut self.pool, key, chunk);
                    self.base.cache_unsaved.insert(key);
                }
                Some(ThreadInput::RawRemoveAck(keys)) => {
                    let write = self.config.database.begin_write()?;
                    let mut save = ChunkSave::new(&write)?;
                    for key in keys {
                        self.unload_raw(&mut save, key)?;
                    }
                    self.pool.limit();
                    drop(save);
                    write.commit()?;
                }
                Some(ThreadInput::CacheRemoveAck(keys)) => {
                    let write = self.config.database.begin_write()?;
                    let mut save = ChunkSave::new(&write)?;
                    for key in keys {
                        self.unload_cache(&mut save, key)?;
                    }
                    self.pool.limit();
                    drop(save);
                    write.commit()?;
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

                    self.load()?;
                    self.unload_ask_overflow()?;

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
Queue: length {} - pending {}
Raw: real {}/{CHUNK_RAW_REAL_CAP} | total {}/{CHUNK_RAW_HARD_CAP}
Cache: real {}/{CHUNK_CACHE_REAL_CAP} | total {}/{CHUNK_CACHE_HARD_CAP}
Pool: {}/{CHUNK_POOL_CAP}
Camera: {:?}",
                self.queue.cache.len(),
                self.queue.cache.len() - self.queue.cache_front,
                self.base.raw_real_cnt,
                self.base.raw.len(),
                self.base.cache_real_cnt,
                self.base.cache.len(),
                self.pool.list.len(),
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
                    let result = match read.load_cache((self.config.page, key)) {
                        Ok(result) => result,
                        Err(err) => {
                            log::error!("loading {key:?} from database error: {err}");
                            ChunkLoadResult {
                                texture: None,
                                associated: false,
                                unsaved: true,
                            }
                        }
                    };
                    if let Some(bytes) = result.texture {
                        let chunk = self.pool.create_dirty_chunk(
                            &self.config.lp,
                            self.config.chunk_size,
                            chunks_stack_key_to_rect(key, self.config.chunk_size),
                        );
                        write_texture(&self.config, &bytes, &chunk.texture);
                        let chunk = CacheLayerChunk::Filled(chunk);

                        self.base.cache_insert(key, chunk.clone());
                        self.output_tx.send(ThreadOutput::CacheInsert(key, chunk))?;
                    } else {
                        self.base.cache_insert(key, CacheLayerChunk::Transparent);
                        self.output_tx
                            .send(ThreadOutput::CacheInsert(key, CacheLayerChunk::Transparent))?;
                    }
                    if result.unsaved {
                        self.base.cache_unsaved.insert(key);
                    }
                }
                ChunkKeyKind::Raw(key) => {
                    let result = match read.load_raw((self.config.page, key)) {
                        Ok(result) => result,
                        Err(err) => {
                            log::error!("loading {key:?} from database error: {err}");
                            ChunkLoadResult {
                                texture: None,
                                associated: false,
                                unsaved: true,
                            }
                        }
                    };
                    if let Some(bytes) = result.texture {
                        let chunk = self.pool.create_dirty_chunk(
                            &self.config.lp,
                            self.config.chunk_size,
                            chunks_key_to_rect(key, self.config.chunk_size),
                        );
                        write_texture(&self.config, &bytes, &chunk.texture);
                        let chunk = RawLayerChunk::Filled(chunk);

                        self.base.raw_insert(key, chunk.clone());
                        self.output_tx.send(ThreadOutput::RawInsert(key, chunk))?;
                    } else if result.associated {
                        self.base.raw_insert(key, RawLayerChunk::Associated);
                        self.output_tx
                            .send(ThreadOutput::RawInsert(key, RawLayerChunk::Associated))?;
                    } else {
                        self.base.raw_insert(key, RawLayerChunk::Transparent);
                        self.output_tx
                            .send(ThreadOutput::RawInsert(key, RawLayerChunk::Transparent))?;
                    }
                    if result.unsaved {
                        self.base.raw_unsaved.insert(key);
                    }
                }
            }
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

    /// only do it when main thread request
    fn unload_all(&mut self) -> Result<(), Box<dyn Error>> {
        self.base.forget_asked();

        let write = self.config.database.begin_write()?;
        let mut save = ChunkSave::new(&write)?;

        while let Some((&key, _)) = self.base.cache.last() {
            self.unload_cache(&mut save, key)?;
        }
        while let Some((&key, _)) = self.base.raw.last() {
            self.unload_raw(&mut save, key)?;
        }

        self.pool.limit();
        drop(save);
        write.commit()?;

        Ok(())
    }

    fn unload_ask_overflow(&mut self) -> Result<(), Box<dyn Error>> {
        self.base.forget_asked();

        if !self.base.cache_overflow() && !self.base.raw_overflow() {
            return Ok(());
        }

        self.base.sort(self.camera.center);

        let mut frnt = self.base.cache.len();
        while frnt > 0 && self.base.cache_overflow() {
            frnt -= 1;
            let (&key, _) = self.base.cache.get_index(frnt).unwrap();
            if (self.queue.cache).contains(&key) {
                continue;
            }

            self.base.cache_removal_asked(key);
            self.output_tx.send(ThreadOutput::CacheRemoveAsk(key))?;
        }

        let mut frnt = self.base.raw.len();
        while frnt > 0 && self.base.raw_overflow() {
            frnt -= 1;
            let (&key, _) = self.base.raw.get_index(frnt).unwrap();
            if (self.queue.raw).contains(&key) {
                continue;
            }

            self.base.raw_removal_asked(key);
            self.output_tx.send(ThreadOutput::RawRemoveAsk(key))?;
        }

        Ok(())
    }

    /// only available with ack from main thread
    fn unload_raw(&mut self, save: &mut ChunkSave, key: (i32, i32)) -> Result<(), Box<dyn Error>> {
        let Some(chunk) = self.base.raw_swap_remove(key) else {
            return Ok(());
        };

        if self.base.raw_unsaved.contains(&key) {
            self.persist_raw(save, key, &chunk)?;
            self.base.raw_unsaved.remove(&key);
        }

        if let RawLayerChunk::Filled(chunk) | RawLayerChunk::AssociatedCached(chunk) = chunk {
            self.pool.list.push(chunk);
        }

        Ok(())
    }

    /// only available with ack from main thread
    fn unload_cache(
        &mut self,
        save: &mut ChunkSave,
        key: (i32, i32, u8),
    ) -> Result<(), Box<dyn Error>> {
        let Some(chunk) = self.base.cache_swap_remove(key) else {
            return Ok(());
        };

        if self.base.cache_unsaved.contains(&key) {
            self.persist_cache(save, key, &chunk)?;
            self.base.cache_unsaved.remove(&key);
        }

        if let CacheLayerChunk::Filled(chunk) = chunk {
            self.pool.list.push(chunk);
        }

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
            RawLayerChunk::Associated | RawLayerChunk::AssociatedCached(_) => {
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
        if let RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_) = chunk {
            self.raw_real_cnt += 1;
        }
        let old = self.raw.insert(key, chunk);
        if let Some(RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_)) = old {
            self.raw_real_cnt -= 1;
        }
    }

    fn cache_insert(&mut self, key: (i32, i32, u8), chunk: CacheLayerChunk) {
        if let CacheLayerChunk::Filled(_) = chunk {
            self.cache_real_cnt += 1;
        }
        let old = self.cache.insert(key, chunk);
        if let Some(CacheLayerChunk::Filled(_)) = old {
            self.cache_real_cnt -= 1;
        }
    }

    fn raw_insert_recycle(
        &mut self,
        pool: &mut DirtyChunkPool,
        key: (i32, i32),
        chunk: RawLayerChunk,
    ) {
        let real = if let RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_) = chunk {
            self.raw_real_cnt += 1;
            true
        } else {
            false
        };
        let old = self.raw.insert(key, chunk);
        if let Some(RawLayerChunk::Filled(chunk) | RawLayerChunk::AssociatedCached(chunk)) = old {
            self.raw_real_cnt -= 1;
            if !real {
                pool.list.push(chunk);
            }
        }
    }

    fn cache_insert_recycle(
        &mut self,
        pool: &mut DirtyChunkPool,
        key: (i32, i32, u8),
        chunk: CacheLayerChunk,
    ) {
        let real = if let CacheLayerChunk::Filled(_) = chunk {
            self.cache_real_cnt += 1;
            true
        } else {
            false
        };
        let old = self.cache.insert(key, chunk);
        if let Some(CacheLayerChunk::Filled(chunk)) = old {
            self.cache_real_cnt -= 1;
            if !real {
                pool.list.push(chunk);
            }
        }
    }

    fn raw_removal_asked(&mut self, key: (i32, i32)) {
        if let Some(chunk) = self.raw.get(&key) {
            self.raw_asked_cnt += 1;
            if let RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_) = chunk {
                self.raw_real_asked_cnt += 1;
            }
        }
    }

    fn cache_removal_asked(&mut self, key: (i32, i32, u8)) {
        if let Some(chunk) = self.cache.get(&key) {
            self.cache_asked_cnt += 1;
            if let CacheLayerChunk::Filled(_) = chunk {
                self.cache_real_asked_cnt += 1;
            }
        }
    }

    fn raw_swap_remove(&mut self, key: (i32, i32)) -> Option<RawLayerChunk> {
        let chunk = self.raw.swap_remove(&key)?;
        if let RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_) = chunk {
            self.raw_real_cnt -= 1;
        }
        Some(chunk)
    }

    fn cache_swap_remove(&mut self, key: (i32, i32, u8)) -> Option<CacheLayerChunk> {
        let chunk = self.cache.swap_remove(&key)?;
        if let CacheLayerChunk::Filled(_) = chunk {
            self.cache_real_cnt -= 1;
        }
        Some(chunk)
    }

    fn cache_overflow(&mut self) -> bool {
        (self.cache_real_cnt - self.cache_real_asked_cnt) >= CHUNK_CACHE_REAL_CAP
            || (self.cache.len() - self.cache_asked_cnt) >= CHUNK_CACHE_HARD_CAP
    }

    fn raw_overflow(&mut self) -> bool {
        (self.raw_real_cnt - self.raw_real_asked_cnt) >= CHUNK_RAW_REAL_CAP
            || (self.raw.len() - self.raw_asked_cnt) >= CHUNK_RAW_HARD_CAP
    }

    fn forget_asked(&mut self) {
        self.raw_asked_cnt = 0;
        self.raw_real_asked_cnt = 0;
        self.cache_asked_cnt = 0;
        self.cache_real_asked_cnt = 0;
    }
}

impl LoadQueue {
    /// camera is moved so we restock the stream queue
    fn restock(&mut self, config: &StreamConfig, camera: &CameraInfo) {
        self.outdated = false;
        self.raw_front = 0;
        self.raw.clear();
        self.cache_front = 0;
        self.cache.clear();

        if camera.center.2 <= 1 {
            for key in rect_chunks_iter(camera.rect, config.chunk_size) {
                self.raw.insert(key);
            }
        }

        for mipmap in camera.center.2..config.mipmap_caps {
            for key in rect_chunks_stack_layer_iter(camera.rect, config.chunk_size, mipmap) {
                self.cache.insert(key);
            }
        }

        // TODO overflow detect

        self.raw
            .sort_by_key(|&key| chunk_distance((key.0, key.1, 0), camera.center));
        self.cache
            .sort_by_key(|&key| chunk_distance(key, camera.center));
    }

    /// select certain amount of chunks defined in [`CHUNK_BATCH`] from queue to load in single batch,
    fn generate_batch(&mut self, base: &mut ChunksBase) {
        debug_assert!(self.batch.is_empty(), "batch is not cleared yet");

        // prioritize cache, render is important
        let mut batch_cnt = 0;
        while let Some(&key) = self.cache.get_index(self.cache_front)
            && batch_cnt < CHUNK_LOAD_BATCH_CAP
        {
            self.cache_front += 1;
            if base.cache.contains_key(&key) {
                continue;
            }

            self.batch.push(ChunkKeyKind::Cache(key));
            batch_cnt += 1;
        }
        while let Some(&key) = self.raw.get_index(self.raw_front)
            && batch_cnt < CHUNK_LOAD_BATCH_CAP
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
        self.cache_front < self.cache.len() || self.raw_front < self.raw.len()
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
fn chunk_distance((x, y, z): (i32, i32, u8), (cx, cy, cz): (i32, i32, u8)) -> u64 {
    let dx = (x << z).abs_diff(cx << cz).saturating_sub(1 << z) as u64;
    let dy = (y << z).abs_diff(cy << cz).saturating_sub(1 << z) as u64;
    (dx + dy << 8) + (255 - z) as u64
}

#[cfg(test)]
mod test {
    #[test]
    fn chunk_order_overlap() {
        let golden: [(i32, i32, u8); _] = [
            (0, 1, 1),
            (0, 0, 1),
            (0, 2, 0),
            (0, 1, 0),
            (1, 1, 0),
            (0, 0, 0),
            (1, 0, 0),
        ];

        let center: (i32, i32, u8) = (0, 2, 0);
        let mut test = golden.clone();
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center));
        assert_eq!(test, golden);
    }

    #[test]
    fn chunk_order_random() {
        // raw data from script/gen/chunks.py
        let golden: [(i32, i32, u8); _] = [
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

        let center: (i32, i32, u8) = (0, 0, 0);
        let mut test = golden.clone();
        test.sort_by_key(|&chunk| super::chunk_distance(chunk, center));
        assert_eq!(test, golden);
    }

    #[test]
    fn chunk_order_key() {
        // raw data from script/gen/chunks.py
        let golden: [((i32, i32, u8), u64); _] = [
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

        let center: (i32, i32, u8) = (-2, 1, 1);
        let mut test = golden.clone();
        test.iter_mut()
            .for_each(|(chunk, key)| *key = super::chunk_distance(*chunk, center));
        test.sort_by_key(|(_, key)| *key);
        assert_eq!(test, golden);
    }
}
