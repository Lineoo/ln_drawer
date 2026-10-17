use std::error::Error;

use redb::{ReadOnlyTable, ReadTransaction, Table, TableDefinition, WriteTransaction};

const CHUNK_VERSION: u32 = 0;

pub const TABLE_LAYER_CHUNK: TableDefinition<(u64, (i32, i32, u8)), &[u8]> =
    TableDefinition::new("layer_chunk");
pub const TABLE_LAYER_CHUNK_RAW: TableDefinition<(u64, (i32, i32)), &[u8]> =
    TableDefinition::new("layer_chunk_raw");

pub struct ChunkLoad {
    cache: ReadOnlyTable<(u64, (i32, i32, u8)), &'static [u8]>,
    raw: ReadOnlyTable<(u64, (i32, i32)), &'static [u8]>,
}

pub struct ChunkSave<'txn> {
    cache: Table<'txn, (u64, (i32, i32, u8)), &'static [u8]>,
    raw: Table<'txn, (u64, (i32, i32)), &'static [u8]>,
}

pub struct ChunkLoadResult {
    pub texture: Option<Vec<u8>>,
    pub associated: bool,
    pub unsaved: bool,
}

impl ChunkLoad {
    pub fn new(read: &ReadTransaction) -> Result<ChunkLoad, Box<dyn Error>> {
        let cache = read.open_table(TABLE_LAYER_CHUNK)?;
        let raw = read.open_table(TABLE_LAYER_CHUNK_RAW)?;
        Ok(ChunkLoad { cache, raw })
    }

    pub fn load_raw(&self, key: (u64, (i32, i32))) -> Result<ChunkLoadResult, Box<dyn Error>> {
        if let Some(entry) = self.raw.get(key)? {
            let bytes = entry.value();
            let Some((version, bytes)) = bytes
                .split_first_chunk::<4>()
                .map(|(x, l)| (u32::from_le_bytes(*x), l))
            else {
                return Err("()".into());
            };
            let Some((associated, texture)) = bytes.split_first().map(|(x, l)| (*x > 0, l)) else {
                return Err("()".into());
            };

            match version {
                0 => Ok(ChunkLoadResult {
                    texture: match associated {
                        false => Some(zstd::decode_all(texture)?),
                        true => None,
                    },
                    associated,
                    unsaved: false,
                }),
                _ => Err("not supported version".into()),
            }
        } else {
            Ok(ChunkLoadResult {
                texture: None,
                associated: false,
                unsaved: false,
            })
        }
    }

    pub fn load_cache(
        &self,
        key: (u64, (i32, i32, u8)),
    ) -> Result<ChunkLoadResult, Box<dyn Error>> {
        if let Some(entry) = self.cache.get(key)? {
            let bytes = entry.value();
            let Some((version, texture)) = bytes
                .split_first_chunk::<4>()
                .map(|(x, l)| (u32::from_le_bytes(*x), l))
            else {
                return Err("()".into());
            };

            match version {
                0 => Ok(ChunkLoadResult {
                    texture: Some(zstd::decode_all(texture)?),
                    associated: false,
                    unsaved: false,
                }),
                _ => Err("not supported version".into()),
            }
        } else {
            Ok(ChunkLoadResult {
                texture: None,
                associated: false,
                unsaved: false,
            })
        }
    }
}

impl ChunkSave<'_> {
    pub fn new(write: &WriteTransaction) -> Result<ChunkSave<'_>, redb::Error> {
        let cache = write.open_table(TABLE_LAYER_CHUNK)?;
        let raw = write.open_table(TABLE_LAYER_CHUNK_RAW)?;
        Ok(ChunkSave { cache, raw })
    }

    pub fn save_raw(&mut self, key: (u64, (i32, i32)), texture: &[u8]) -> Result<(), redb::Error> {
        if self.is_transparent(texture) {
            return self.save_raw_transparent(key);
        }

        let mut texture = zstd::encode_all(texture, 0)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        bytes.push(0);
        bytes.append(&mut texture);
        self.raw.insert(key, &bytes[..])?;

        Ok(())
    }

    pub fn save_raw_associated(&mut self, key: (u64, (i32, i32))) -> Result<(), redb::Error> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        bytes.push(1);
        self.raw.insert(key, &bytes[..])?;
        Ok(())
    }

    pub fn save_raw_transparent(&mut self, key: (u64, (i32, i32))) -> Result<(), redb::Error> {
        self.raw.remove(key)?;
        Ok(())
    }

    pub fn save_cache(
        &mut self,
        key: (u64, (i32, i32, u8)),
        texture: &[u8],
    ) -> Result<(), redb::Error> {
        if self.is_transparent(texture) {
            return self.save_cache_transparent(key);
        }

        let mut texture = zstd::encode_all(texture, 0)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        bytes.append(&mut texture);
        self.cache.insert(key, &bytes[..])?;

        Ok(())
    }

    pub fn save_cache_transparent(
        &mut self,
        key: (u64, (i32, i32, u8)),
    ) -> Result<(), redb::Error> {
        self.cache.remove(key)?;
        Ok(())
    }

    pub fn is_transparent(&self, texture: &[u8]) -> bool {
        for &byte in texture {
            if byte != 0x00 {
                return false;
            }
        }
        true
    }
}

pub mod legacy_classic {
    use std::error::Error;

    use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

    use crate::{
        color::srgb::{srgb_gamma_decode_f32, srgb_gamma_encode_f32},
        save::chunk::ChunkSave,
    };

    type ChunkKey = (i32, i32, u8);

    #[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
    struct ChunkMeta0 {
        format: u32,
    }

    const LEGACY_TABLE_LAYER_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
        TableDefinition::new("stroke_chunk");
    const LEGACY_TABLE_LAYER_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
        TableDefinition::new("stroke_chunk_meta");
    const LEGACY_TABLE_DATABASE_METADATA: TableDefinition<u32, &[u8]> =
        TableDefinition::new("metadata");
    const LEGACY_TABLE_CAMERA: TableDefinition<&str, &[u8]> = TableDefinition::new("camera");

    pub fn need_migration(db: &Database) -> Result<bool, redb::Error> {
        let read = db.begin_read()?;
        if read.open_table(LEGACY_TABLE_LAYER_CHUNK).is_err() {
            // no need to migrate
            return Ok(false);
        }
        drop(read);
        Ok(true)
    }

    pub fn startup_migration(db: &mut Database) -> Result<(), redb::Error> {
        log::warn!("migration from classic database");

        let mut write = db.begin_write()?;
        write.set_quick_repair(true);
        let legacy_chunk_table = write.open_table(LEGACY_TABLE_LAYER_CHUNK)?;
        let legacy_meta_table = write.open_table(LEGACY_TABLE_LAYER_CHUNK_META)?;
        let mut save = ChunkSave::new(&write)?;

        for pair in legacy_chunk_table.iter()? {
            let (key, chunk) = pair?;
            let key = key.value();
            // lack meta will use version 0 due to some old bugs
            let meta0 = match legacy_meta_table.get(&(key, 0))? {
                Some(access) => {
                    let bytes = access.value();
                    postcard::from_bytes::<ChunkMeta0>(bytes).unwrap_or_default()
                }
                None => ChunkMeta0::default(),
            };
            let mut bytes = zstd::decode_all(chunk.value()).unwrap();
            chunk_migration(&mut bytes[..], meta0.format).unwrap();
            save.save_cache(key, &bytes[..])?;
            if key.1.2 == 0 {
                save.save_raw_associated((key.0, (key.1.0, key.1.1)))?;
            }
        }

        drop(legacy_chunk_table);
        drop(legacy_meta_table);
        drop(save);
        write.delete_table(LEGACY_TABLE_LAYER_CHUNK)?;
        write.delete_table(LEGACY_TABLE_LAYER_CHUNK_META)?;
        write.delete_table(LEGACY_TABLE_DATABASE_METADATA)?;
        write.delete_table(LEGACY_TABLE_CAMERA)?;
        write.commit()?;
        db.compact()?;

        log::warn!("finish migration");

        Ok(())
    }

    fn chunk_migration(bytes: &mut [u8], from: u32) -> Result<(), Box<dyn Error + 'static>> {
        for migrate_format in from..3 {
            match migrate_format {
                0 => {
                    let (chunks, _) = bytes.as_chunks_mut();
                    for [r, g, b, _] in chunks {
                        *r = (srgb_gamma_encode_f32(*r as f32 / 255.) * 255.) as u8;
                        *g = (srgb_gamma_encode_f32(*g as f32 / 255.) * 255.) as u8;
                        *b = (srgb_gamma_encode_f32(*b as f32 / 255.) * 255.) as u8;
                    }
                }
                1 => {
                    // remove deprecated mipmap field, migration is a no-op
                }
                2 => {
                    let (chunks, _) = bytes.as_chunks_mut();
                    for [r, g, b, a] in chunks {
                        *r = (srgb_gamma_encode_f32(
                            srgb_gamma_decode_f32(*r as f32 / 255.) * (*a as f32 / 255.),
                        ) * 255.) as u8;
                        *g = (srgb_gamma_encode_f32(
                            srgb_gamma_decode_f32(*g as f32 / 255.) * (*a as f32 / 255.),
                        ) * 255.) as u8;
                        *b = (srgb_gamma_encode_f32(
                            srgb_gamma_decode_f32(*b as f32 / 255.) * (*a as f32 / 255.),
                        ) * 255.) as u8;
                    }
                }
                _ => unreachable!("unsupported migration {migrate_format}"),
            }
        }
        Ok(())
    }
}
