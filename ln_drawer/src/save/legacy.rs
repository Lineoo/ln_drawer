use redb::{
    MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};
use serde_bytes::ByteBuf;

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
