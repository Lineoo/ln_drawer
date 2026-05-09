struct Chunk {
    chunk: vec3i
}

struct MipmapMeta {
    mipmap_coords: vec2i,
    mipmap_size: vec2u,
}

@group(0) @binding(0) var<uniform> mipmapping: MipmapMeta;

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination_key: Chunk;
@group(1) @binding(2) var source: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> source_key: Chunk;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if id.x > mipmapping.mipmap_size.x || id.y > mipmapping.mipmap_size.y { return; }

    let coords = mipmapping.mipmap_coords + vec2i(id.xy);
    let smol = coords % 256 * 2;

    let sum = textureLoad(source, smol)
        + textureLoad(source, smol + vec2i(0, 1))
        + textureLoad(source, smol + vec2i(1, 1))
        + textureLoad(source, smol + vec2i(1, 0));

    textureStore(destination, coords, sum / 4);
}