use std::{
    error::Error,
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
};

use hashbrown::HashSet;
use indexmap::{IndexMap, IndexSet};
use redb::ReadableDatabase;
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Extent3d, MapMode, Origin3d,
    PollType, Queue, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

use crate::{
    measures::{Position, Rectangle, Size},
    render::camera::Camera,
    save::SaveDatabase,
    stroke::{
        CHUNK_BATCH, CHUNK_CAPS, CHUNK_MIPMAP, CHUNK_SIZE, ChunkKey, TABLE_STROKE_CHUNK,
        ThreadInput, ThreadOutput, chunk_distance, chunk_of, chunks_within,
    },
};

pub fn loading_thread(
    database: SaveDatabase,
    device: Device,
    queue: Queue,
    input_rx: Receiver<ThreadInput>,
    output_tx: Sender<ThreadOutput>,
) -> Result<(), Box<dyn Error>> {
    let mut texel = IndexMap::<ChunkKey, Option<Texture>>::new();
    let mut texel_staging = IndexSet::<ChunkKey>::new();
    let mut texel_unsaved = HashSet::new();

    let mut stream_center = (0, 0, 0);
    let mut stream_rect = Rectangle::new_half(Position::ZERO, Size::splat(50));
    let mut stream_range = chunks_within(stream_rect, 0);
    let mut stream_outdated = false;

    let mut stream_front = 0;
    let mut stream_queue = IndexSet::with_capacity(400);

    loop {
        let input = if stream_front < stream_queue.len() || stream_outdated {
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
                stream_rect = Camera::manual_view_rect(zoom, size, center);
                let stream_center_new = chunk_of(center.round(), zoom);
                let stream_range_new = chunks_within(stream_rect, stream_center.2);
                if stream_range_new != stream_range || stream_center_new != stream_center {
                    stream_range = stream_range_new;
                    stream_center = stream_center_new;
                    stream_outdated = true;
                }
                continue;
            }
            Some(ThreadInput::MarkUnsaved(chunk)) => {
                texel_unsaved.insert(chunk);
                continue;
            }
            Some(ThreadInput::Create(chunk_id, texture)) => {
                // this happen when main thread doesn't receive Remove signal when
                // our thread already unload the chunk. Ignoring it is okay, though
                // a few changes may not be saved.
                debug_assert!(texel.get(&chunk_id).is_some_and(|x| x.is_none()));
                texel.insert(chunk_id, Some(texture));
                continue;
            }
            Some(ThreadInput::Autosave) => {
                let write = database.0.begin_write()?;
                {
                    let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
                    for key in texel_unsaved.drain() {
                        let Some(Some(texture)) = texel.get(&key) else {
                            continue;
                        };

                        let bytes = chunk_readback(&texture, &device, &queue);
                        let compressed = zstd::encode_all(&bytes[..], 0)?;
                        table_chunk.insert((0, key), &compressed[..])?;
                    }
                }
                write.commit()?;
                continue;
            }
            Some(ThreadInput::Finish) => {
                return Ok(());
            }
            None => {}
        };

        // Stream
        if stream_outdated {
            stream_outdated = false;
            stream_front = 0;
            stream_queue.clear();

            for z in stream_center.2.saturating_sub(1)..CHUNK_MIPMAP {
                let (range_src, range_dst) = chunks_within(stream_rect, z);
                for x in range_src.0..range_dst.0 {
                    for y in range_src.1..range_dst.1 {
                        stream_queue.insert((x, y, z));
                    }
                }
            }

            debug_assert!(stream_queue.len() < CHUNK_CAPS - 1);

            stream_queue.sort_by_key(|&(x, y, z)| {
                chunk_distance(x, y, z, stream_center.0, stream_center.1, stream_center.2)
            });
        }

        // Assign loading
        let mut batch_cnt = 0;
        while let Some(&key) = stream_queue.get_index(stream_front)
            && batch_cnt < CHUNK_BATCH
        {
            stream_front += 1;
            if texel.contains_key(&key) {
                continue;
            }

            texel.insert(key, None);
            texel_staging.insert(key);
            batch_cnt += 1;
        }

        // Early exiting
        if texel_staging.is_empty() {
            continue;
        }

        // Unloading
        texel.sort_by_key(|&(x, y, z), _| {
            chunk_distance(x, y, z, stream_center.0, stream_center.1, stream_center.2)
        });
        let write = database.0.begin_write()?;
        let mut table_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
        let mut frnt = texel.len();
        while texel.len() + texel_staging.len() >= CHUNK_CAPS {
            frnt -= 1;
            if stream_queue.contains(texel.get_index(frnt).unwrap().0) {
                continue;
            }

            let (key, texture) = texel.swap_remove_index(frnt).unwrap();
            output_tx.send(ThreadOutput::Remove(key))?;

            if let Some(texture) = texture
                && texel_unsaved.remove(&key)
            {
                let bytes = chunk_readback(&texture, &device, &queue);
                let compressed = zstd::encode_all(&bytes[..], 0)?;
                table_chunk.insert((0, key), &compressed[..])?;
            }
        }
        drop(table_chunk);
        write.commit()?;

        // Loading
        let read = database.0.begin_read()?;
        let table_chunk = read.open_table(TABLE_STROKE_CHUNK)?;
        for chunk_id in texel_staging.drain(..) {
            if let Some(chunk) = table_chunk.get((0, chunk_id))? {
                let bytes = zstd::decode_all(chunk.value())?;

                let texture = device.create_texture(&TextureDescriptor {
                    label: Some("stroke_chunk_texture"),
                    size: Extent3d {
                        width: CHUNK_SIZE,
                        height: CHUNK_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8Unorm,
                    usage: TextureUsages::COPY_SRC
                        | TextureUsages::COPY_DST
                        | TextureUsages::TEXTURE_BINDING
                        | TextureUsages::STORAGE_BINDING,
                    view_formats: &[],
                });

                queue.write_texture(
                    TexelCopyTextureInfoBase {
                        texture: &texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &bytes,
                    TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(CHUNK_SIZE * 4),
                        rows_per_image: Some(CHUNK_SIZE),
                    },
                    Extent3d {
                        width: CHUNK_SIZE,
                        height: CHUNK_SIZE,
                        depth_or_array_layers: 1,
                    },
                );

                texel.insert(chunk_id, Some(texture.clone()));
                output_tx.send(ThreadOutput::Insert(chunk_id, Some(texture)))?;
            } else {
                texel.insert(chunk_id, None);
                output_tx.send(ThreadOutput::Insert(chunk_id, None))?;
            }
        }
    }
}

fn chunk_readback(texture: &Texture, device: &Device, queue: &Queue) -> Vec<u8> {
    let (tx, rx) = std::sync::mpsc::channel();

    let readback_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("chunk_readback"),
        size: (CHUNK_SIZE * CHUNK_SIZE * 4) as u64,
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
                bytes_per_row: Some(CHUNK_SIZE * 4),
                rows_per_image: Some(CHUNK_SIZE),
            },
        },
        Extent3d {
            width: CHUNK_SIZE,
            height: CHUNK_SIZE,
            depth_or_array_layers: 1,
        },
    );

    let command = encoder.finish();

    queue.submit([command]);

    let inner = readback_buffer.clone();
    readback_buffer.map_async(MapMode::Read, .., move |ret| {
        ret.unwrap();

        let view = inner.get_mapped_range(..);
        tx.send(view.to_vec()).unwrap();
    });

    device.poll(PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap()
}
