struct MipmapMeta {
    mipmap_coords: vec2i,
    mipmap_size: vec2u,
}

@group(0) @binding(0) var<uniform> mipmapping: MipmapMeta;

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination_key: vec3i;
@group(1) @binding(2) var source: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(3) var<uniform> source_key: vec3i;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let texl_size = i32(exp2(f32(destination_key.z)));
    let real_size = texture_size * texl_size;
    let chunk_min = (destination_key.xy) * real_size;

    let src_texl_size = i32(exp2(f32(source_key.z)));
    let src_real_size = texture_size * src_texl_size;
    let src_chunk_min = (source_key.xy) * src_real_size;
    let src_chunk_max = (source_key.xy + vec2i(1)) * src_real_size;

    let area_min = mipmapping.mipmap_coords;
    let area_max = mipmapping.mipmap_coords + vec2i(mipmapping.mipmap_size);

    let coords_min = (real_size + area_min - chunk_min) / texl_size - texture_size;
    let coords_max = (real_size + area_max - chunk_min - 1) / texl_size + 1 - texture_size;

    let area = area_min + vec2i(id.xy) * texl_size;
    let coords = coords_min + vec2i(id.xy);

    if coords.x >= coords_max.x || coords.y >= coords_max.y { return; }
    if area.x < src_chunk_min.x || area.y < src_chunk_min.y || area.x >= src_chunk_max.x || area.y >= src_chunk_max.y { return; }

    let smol = coords % 256 * 2;
    let sum = textureLoad(source, smol)
        + textureLoad(source, smol + vec2i(0, 1))
        + textureLoad(source, smol + vec2i(1, 1))
        + textureLoad(source, smol + vec2i(1, 0));

    textureStore(destination, coords, sum / 4);
}