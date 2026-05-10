struct MipmapMeta {
    mipmap_coords: vec2i,
    mipmap_size: vec2u,
}

@group(0) @binding(0) var<uniform> mipmapping: MipmapMeta;

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination_key: vec3i;
@group(1) @binding(2) var source: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> source_key: vec3i;

const chunk_size: u32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let coords = vec2u(mipmapping.mipmap_coords) / u32(exp2(f32(destination_key.z))) % chunk_size + id.xy;
    if coords.x > chunk_size || coords.y > chunk_size { return; }

    let smol = coords % 256 * 2;
    let sum = textureLoad(source, smol)
        + textureLoad(source, smol + vec2u(0, 1))
        + textureLoad(source, smol + vec2u(1, 1))
        + textureLoad(source, smol + vec2u(1, 0));

    textureStore(destination, coords, sum / 4);
}