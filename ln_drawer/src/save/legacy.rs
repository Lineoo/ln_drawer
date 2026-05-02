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
