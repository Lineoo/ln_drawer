use std::{error::Error, path::Path};

use redb::{
    MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};
use serde_bytes::ByteBuf;

use super::chunk::{CHUNK_FORMAT_CURRENT, ChunkStore, chunk_migration, chunk_root};
use crate::layer::ChunkKey;

/// Deprecate `SaveControl` and give custom tables to callers themselves to handle with.
///
/// This migration will move StrokeLayer's chunks from main control table to their custom table.
pub fn migrate0(write: &WriteTransaction) -> Result<(), redb::Error> {
    const LEGACY_TABLE_CONTROLS: TableDefinition<u64, &[u8]> = TableDefinition::new("controls");
    const LEGACY_TABLE_CONTROLS_LUT_CLASS: MultimapTableDefinition<&str, u64> =
        MultimapTableDefinition::new("controls_lut_class");
    const LEGACY_TABLE_CONTROLS_LUT_WITHIN: MultimapTableDefinition<(&str, u64), u64> =
        MultimapTableDefinition::new("controls_lut_within");

    const TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
        MultimapTableDefinition::new("stroke");
    const TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> =
        TableDefinition::new("stroke_chunk");

    #[derive(serde::Serialize, serde::Deserialize)]
    struct LegacyChunkArchive {
        chunk: (i32, i32),
        bytes: ByteBuf,
    }

    // migrate old data
    {
        let controls = write.open_table(LEGACY_TABLE_CONTROLS)?;
        let class = write.open_multimap_table(LEGACY_TABLE_CONTROLS_LUT_CLASS)?;
        let mut stroke = write.open_multimap_table(TABLE_STROKE)?;
        let mut stroke_chunk = write.open_table(TABLE_STROKE_CHUNK)?;
        for chunk in class.get("canvas_chunk")? {
            let bytes = controls.get(chunk?.value())?.unwrap();
            let bytes = zstd::decode_all(bytes.value()).unwrap();
            let archive = postcard::from_bytes::<LegacyChunkArchive>(&bytes[..]).unwrap();
            let compressed = zstd::encode_all(&archive.bytes[..], 0).unwrap();
            stroke.insert((), archive.chunk)?;
            stroke_chunk.insert(archive.chunk, &compressed[..])?;
        }
    }

    // clean up old table
    {
        write.delete_table(LEGACY_TABLE_CONTROLS)?;
        write.delete_multimap_table(LEGACY_TABLE_CONTROLS_LUT_CLASS)?;
        write.delete_multimap_table(LEGACY_TABLE_CONTROLS_LUT_WITHIN)?;
    }

    Ok(())
}

/// Add mipmap level key and stroke layer key, remove main chunk index table. The
/// rest of completing mipmaps will be done by the stroke layer, where exists another
/// process of migrating that supports stream upgrading instead of full-upgrade.
///
/// This migration will add mipmap level marker 0 and layer identity 0 to all StrokeLayer's
/// chunks, delete unused index chunk table, and add a stroke meta0 table.
pub fn migrate1(write: &WriteTransaction) -> Result<(), redb::Error> {
    const LEGACY_TABLE_STROKE: MultimapTableDefinition<(), (i32, i32)> =
        MultimapTableDefinition::new("stroke");
    const LEGACY_TABLE_STROKE_CHUNK: TableDefinition<(i32, i32), &[u8]> =
        TableDefinition::new("stroke_chunk");

    const BUFFER_TABLE: TableDefinition<(i32, i32), &[u8]> = TableDefinition::new("_temp_");

    const TABLE_STROKE_CHUNK: TableDefinition<(u64, (i32, i32, u8)), &[u8]> =
        TableDefinition::new("stroke_chunk");
    const TABLE_STROKE_CHUNK_META: TableDefinition<((u64, (i32, i32, u8)), u32), &[u8]> =
        TableDefinition::new("stroke_chunk_meta");

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
    struct ChunkMeta0 {
        format: u32,
        mipmapped: u8,
        _pad: [u8; 3],
    }

    // migrate data
    {
        write.rename_table(LEGACY_TABLE_STROKE_CHUNK, BUFFER_TABLE)?;
        let legacy = write.open_table(BUFFER_TABLE)?;
        let mut table = write.open_table(TABLE_STROKE_CHUNK)?;
        let mut table_meta0 = write.open_table(TABLE_STROKE_CHUNK_META)?;
        for result in legacy.iter()? {
            let (key, value) = result?;
            let ((x, y), value) = (key.value(), value.value());
            table.insert((0, (x, y, 0)), value)?;
            table_meta0.insert(
                ((0, (x, y, 0)), 0),
                bytemuck::bytes_of(&ChunkMeta0 {
                    format: 0,
                    mipmapped: 0,
                    _pad: [0; 3],
                }),
            )?;
        }
    }

    // clean up buffer table
    {
        write.delete_multimap_table(LEGACY_TABLE_STROKE)?;
        write.delete_table(BUFFER_TABLE)?;
    }

    Ok(())
}

/// Move every canvas chunk out of the main database into the sidecar
/// [`ChunkStore`], then drop the now-unused chunk tables.
///
/// Chunks in the main database made redb's startup allocator scan scale with the
/// whole canvas (~300MB). After this migration chunk blobs live as individual
/// files under `chunks/` next to the database, while redb only keeps small
/// metadata.
///
/// The migration is idempotent: if it is interrupted between deleting the tables
/// and bumping the format version, the next run simply finds empty tables.
#[allow(dead_code)] // TODO(step-3): drop once wired into `migrate_format`.
pub fn migrate2(write: &WriteTransaction, file: &Path) -> Result<(), Box<dyn Error>> {
    const TABLE_LAYER_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
        TableDefinition::new("stroke_chunk");
    const TABLE_LAYER_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
        TableDefinition::new("stroke_chunk_meta");

    #[derive(serde::Deserialize)]
    struct ChunkMeta0 {
        format: u32,
    }

    let store = ChunkStore::open(chunk_root(file))?;

    {
        let table_chunk = write.open_table(TABLE_LAYER_CHUNK)?;
        let table_meta = write.open_table(TABLE_LAYER_CHUNK_META)?;

        for entry in table_chunk.iter()? {
            let (key, value) = entry?;
            let (page, chunk) = key.value();
            let mut bytes = zstd::decode_all(value.value())?;

            // Format values 0 and 1 share their low byte under both the historic
            // bytemuck layout and the postcard layout, and postcard ignores the
            // trailing deprecated `_mipmapped` byte, so this reads either.
            let format = table_meta
                .get(((page, chunk), 0))?
                .and_then(|meta| postcard::from_bytes::<ChunkMeta0>(meta.value()).ok())
                .map_or(0, |meta| meta.format);

            if format < CHUNK_FORMAT_CURRENT {
                chunk_migration(&mut bytes, chunk, format);
            }

            store.write(page, chunk, &bytes)?;
        }
    }

    write.delete_table(TABLE_LAYER_CHUNK)?;
    write.delete_table(TABLE_LAYER_CHUNK_META)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::{fs, path::PathBuf};

    use redb::{Database, ReadableDatabase, TableDefinition};

    use super::migrate2;
    use crate::{
        layer::ChunkKey,
        save::chunk::{ChunkStore, chunk_root},
    };

    const TABLE_LAYER_CHUNK: TableDefinition<(u64, ChunkKey), &[u8]> =
        TableDefinition::new("stroke_chunk");
    const TABLE_LAYER_CHUNK_META: TableDefinition<((u64, ChunkKey), u32), &[u8]> =
        TableDefinition::new("stroke_chunk_meta");

    #[derive(serde::Serialize)]
    struct SerializedMeta0 {
        format: u32,
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ln_drawer_migrate2_test_{}_{}_{}",
                std::process::id(),
                name,
                nanos
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn compressed(bytes: &[u8]) -> Vec<u8> {
        zstd::encode_all(bytes, 0).unwrap()
    }

    fn meta(format: u32) -> Vec<u8> {
        postcard::to_stdvec(&SerializedMeta0 { format }).unwrap()
    }

    #[test]
    fn migrate2_exports_chunks_and_drops_tables() {
        let temp = TempDir::new("export");
        let file = temp.0.join("world.lndb");
        let db = Database::create(&file).unwrap();

        let format0 = [255u8, 128, 0, 255];
        let format1 = [12u8, 34, 56, 78];

        {
            let write = db.begin_write().unwrap();
            {
                let mut table_chunk = write.open_table(TABLE_LAYER_CHUNK).unwrap();
                let mut table_meta = write.open_table(TABLE_LAYER_CHUNK_META).unwrap();

                table_chunk
                    .insert((0, (0, 0, 0)), &compressed(&format0)[..])
                    .unwrap();
                table_meta
                    .insert(((0, (0, 0, 0)), 0), &meta(0)[..])
                    .unwrap();

                table_chunk
                    .insert((2, (-3, 4, 1)), &compressed(&format1)[..])
                    .unwrap();
                table_meta
                    .insert(((2, (-3, 4, 1)), 0), &meta(1)[..])
                    .unwrap();

                // Missing meta must be treated as format 0.
                table_chunk
                    .insert((0, (1, 1, 0)), &compressed(&format0)[..])
                    .unwrap();
            }
            write.commit().unwrap();
        }

        let write = db.begin_write().unwrap();
        migrate2(&write, &file).unwrap();
        write.commit().unwrap();

        let store = ChunkStore::open(chunk_root(&file)).unwrap();

        let migrated = store.read(0, (0, 0, 0)).unwrap().unwrap();
        assert_ne!(migrated[1], format0[1], "format 0 must be gamma migrated");
        assert_eq!(store.read(2, (-3, 4, 1)).unwrap().unwrap(), format1);
        let missing_meta = store.read(0, (1, 1, 0)).unwrap().unwrap();
        assert_ne!(missing_meta[1], format0[1]);

        let read = db.begin_read().unwrap();
        assert!(read.open_table(TABLE_LAYER_CHUNK).is_err());
        assert!(read.open_table(TABLE_LAYER_CHUNK_META).is_err());
        drop(read);

        // Idempotent on a database whose chunk tables are already gone.
        let write = db.begin_write().unwrap();
        migrate2(&write, &file).unwrap();
        write.commit().unwrap();
    }
}
