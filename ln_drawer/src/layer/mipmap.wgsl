#lib_colorspace #lib_rectangle

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: source texture is always aligned sub-mipmap texture
    let scale = source.size / textureDimensions(source_texture) * 2;
    let dispatch_start = align(dispatch.coords, vec2i(scale));
    let dispatch_end = align(dispatch.coords + vec2i(dispatch.size) + vec2i(scale - 1u), vec2i(scale));

    let start = max(max(dispatch_start, destination.coords), source.coords);
    let position = start + vec2i(id.xy * scale);

    let validated = all(position >= dispatch_start) & all(position < dispatch_end)
        && rectangle_contains(destination, position)
        && rectangle_contains(source, position);
    if !validated { return; }

    let dst_coords = (position - destination.coords) / vec2i(scale);
    let src_coords = dst_coords % 256 * 2;

    let c0 = srgb_gamma_decode(textureLoad(source_texture, src_coords));
    let c1 = srgb_gamma_decode(textureLoad(source_texture, src_coords + vec2i(0, 1)));
    let c2 = srgb_gamma_decode(textureLoad(source_texture, src_coords + vec2i(1, 1)));
    let c3 = srgb_gamma_decode(textureLoad(source_texture, src_coords + vec2i(1, 0)));

    textureStore(destination_texture, dst_coords, srgb_gamma_encode((c0 + c1 + c2 + c3) / 4));
}

fn align(v: vec2i, s: vec2i) -> vec2i {
    return v - ((v % s) + s) % s;
}