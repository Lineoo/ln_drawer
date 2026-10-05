use redb::TableDefinition;

const CHUNK_VERSION: u32 = 0;

const TABLE_LAYER_CHUNK: TableDefinition<(u64, (i32, i32, u8)), &[u8]> =
    TableDefinition::new("layer_chunk");
const TABLE_LAYER_CHUNK_RAW: TableDefinition<(u64, (i32, i32)), &[u8]> =
    TableDefinition::new("layer_chunk_raw");

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
}
