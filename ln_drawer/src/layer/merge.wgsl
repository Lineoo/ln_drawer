// include! colorspace

struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let scale = destination.size / textureDimensions(destination_texture);
    let position = dispatch.coords + vec2i(id.xy * scale);

    if (any(position >= dispatch.coords + vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position >= destination.coords + vec2i(destination.size))) { return; }

    if (any(position < source.coords)) { return; }
    if (any(position >= source.coords + vec2i(source.size))) { return; }

    // Assumption: source texture is always same pixel size with destination
    let dst_coords = (position - destination.coords) / vec2i(scale);
    let src_coords = (position - source.coords) / vec2i(scale);

    let dst_ump = srgb_to_linear(textureLoad(destination_texture, dst_coords));
    let src_ump = srgb_to_linear(textureLoad(source_texture, src_coords));

    let dst = vec4f(dst_ump.rgb, 1) * dst_ump.a;
    let src = vec4f(src_ump.rgb, 1) * src_ump.a;

    let fnl = src + dst * (1 - src.a);

    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(fnl.rgb / fnl.a, fnl.a)), vec4f(0), fnl.a < 1e-6));
}

@compute @workgroup_size(16, 16)
fn cs_erase(@builtin(global_invocation_id) id: vec3u) {
    let scale = destination.size / textureDimensions(destination_texture);
    let position = dispatch.coords + vec2i(id.xy * scale);

    if (any(position >= dispatch.coords + vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position >= destination.coords + vec2i(destination.size))) { return; }

    if (any(position < source.coords)) { return; }
    if (any(position >= source.coords + vec2i(source.size))) { return; }

    // Assumption: source texture is always same pixel size with destination
    let dst_coords = (position - destination.coords) / vec2i(scale);
    let src_coords = (position - source.coords) / vec2i(scale);

    let dst_ump = srgb_to_linear(textureLoad(destination_texture, dst_coords));
    let src_ump = srgb_to_linear(textureLoad(source_texture, src_coords));

    let dst = vec4f(dst_ump.rgb, 1) * dst_ump.a;
    let src = vec4f(src_ump.rgb, 1) * src_ump.a;

    let fnl = dst * (1 - src.a);

    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(fnl.rgb / fnl.a, fnl.a)), vec4f(0), fnl.a < 1e-6));
}
