use std::error::Error;

use redb::{ReadOnlyTable, ReadTransaction, Table, TableDefinition, WriteTransaction};

const CHUNK_VERSION: u32 = 0;

const TABLE_LAYER_CHUNK: TableDefinition<(u64, (i32, i32, u8)), &[u8]> =
    TableDefinition::new("layer_chunk");
const TABLE_LAYER_CHUNK_RAW: TableDefinition<(u64, (i32, i32)), &[u8]> =
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
    pub fn new(write: &WriteTransaction) -> Result<ChunkSave<'_>, Box<dyn Error>> {
        let cache = write.open_table(TABLE_LAYER_CHUNK)?;
        let raw = write.open_table(TABLE_LAYER_CHUNK_RAW)?;
        Ok(ChunkSave { cache, raw })
    }

    pub fn save_raw(
        &mut self,
        key: (u64, (i32, i32)),
        texture: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let mut texture = zstd::encode_all(texture, 0)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        bytes.push(0);
        bytes.append(&mut texture);
        self.raw.insert(key, &bytes[..])?;

        Ok(())
    }

    pub fn save_raw_associated(&mut self, key: (u64, (i32, i32))) -> Result<(), Box<dyn Error>> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        bytes.push(1);
        self.raw.insert(key, &bytes[..])?;
        Ok(())
    }

    pub fn save_raw_transparent(&mut self, key: (u64, (i32, i32))) -> Result<(), Box<dyn Error>> {
        self.raw.remove(key)?;
        Ok(())
    }

    pub fn save_cache(
        &mut self,
        key: (u64, (i32, i32, u8)),
        texture: &[u8],
    ) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(), Box<dyn Error>> {
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

mod legacy_classic {
    use redb::TableDefinition;

    type ChunkKey = (i32, i32, u8);

    /// last version used
    /// `2`: 0.4.0
    const CHUNK_META0_FORMAT: u32 = 3;

    const TABLE_LAYER_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
        TableDefinition::new("stroke_chunk");
    const TABLE_LAYER_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
        TableDefinition::new("stroke_chunk_meta");

    // fn chunk_migration(
    //     config: &StreamConfig,
    //     debug: &mut DebugInfo,
    //     key: ChunkKey,
    //     bytes: &mut [u8],
    //     mut meta0: ChunkMeta0,
    // ) -> Result<(), Box<dyn Error + 'static>> {
    //     let mut chunk_changed = false;
    //     for migrate_format in meta0.format..3 {
    //         match migrate_format {
    //             0 => {
    //                 let (chunks, _) = bytes.as_chunks_mut();
    //                 for [r, g, b, _] in chunks {
    //                     *r = (srgb_gamma_encode(*r as f32 / 255.) * 255.) as u8;
    //                     *g = (srgb_gamma_encode(*g as f32 / 255.) * 255.) as u8;
    //                     *b = (srgb_gamma_encode(*b as f32 / 255.) * 255.) as u8;
    //                 }
    //                 chunk_changed = true;
    //             }
    //             1 => {
    //                 // remove deprecated mipmap field, migration is a no-op
    //             }
    //             2 => {
    //                 let (chunks, _) = bytes.as_chunks_mut();
    //                 for [r, g, b, a] in chunks {
    //                     *r = (srgb_gamma_encode(
    //                         srgb_gamma_decode(*r as f32 / 255.) * (*a as f32 / 255.),
    //                     ) * 255.) as u8;
    //                     *g = (srgb_gamma_encode(
    //                         srgb_gamma_decode(*g as f32 / 255.) * (*a as f32 / 255.),
    //                     ) * 255.) as u8;
    //                     *b = (srgb_gamma_encode(
    //                         srgb_gamma_decode(*b as f32 / 255.) * (*a as f32 / 255.),
    //                     ) * 255.) as u8;
    //                 }
    //                 chunk_changed = true;
    //             }
    //             _ => unreachable!("unsupported migration {migrate_format}"),
    //         }
    //     }
    //     Ok(())
    // }
}
