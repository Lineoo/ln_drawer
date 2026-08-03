// include! colorspace

struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(0) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(0) @binding(1) var<uniform> destination: Rectangle;

@group(1) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let scale = destination.size / textureDimensions(destination_texture);

    // Assumption: source texture is always same pixel size with destination
    let src_coords = vec2i(id.xy);
    let dst_coords = src_coords + source.coords - destination.coords;

    if (any(dst_coords < vec2i())) { return; }
    if (any(dst_coords >= vec2i(destination.size))) { return; }

    if (any(src_coords < vec2i())) { return; }
    if (any(src_coords >= vec2i(source.size))) { return; }

    let dst_ump = textureLoad(destination_texture, dst_coords);
    let src_ump = textureLoad(source_texture, src_coords);

    let dst = vec4f(dst_ump.rgb, 1) * dst_ump.a;
    let src = vec4f(src_ump.rgb, 1) * src_ump.a;

    let fnl = composite(src, dst);

    textureStore(destination_texture, dst_coords, select(vec4f(fnl.rgb / fnl.a, fnl.a), vec4f(0), fnl.a < 1e-6));
}
