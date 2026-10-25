# redb 数据库 #

### 表类型速查

| name | key | value |
|------|-----|-------|
| layer_chunk     | (u64, (i32, i32, u8)) | &[u8] |
| layer_chunk_raw | (u64, (i32, i32))     | &[u8] |
| layer_object    | (u64, u128)           | &[u8]  |

### layer_chunk 缓存区块表格式

- **version 版本** 小端 u32 (4 字节)
- **texture 纹理** zstd 编码

### layer_chunk_raw 原始区块表格式

- **version 版本** 小端 u32 (4 字节)
- **associated 伴生图层**  bool (1 字节)
- **texture 纹理** zstd 编码