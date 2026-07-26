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

    // Assumption: source texture is always aligned sub-mipmap texture
    let dst_coords = (position - destination.coords) / vec2i(scale);
    let src_coords = dst_coords % 256 * 2;

    let c0_ump = srgb_to_linear(textureLoad(source_texture, src_coords));
    let c1_ump = srgb_to_linear(textureLoad(source_texture, src_coords + vec2i(0, 1)));
    let c2_ump = srgb_to_linear(textureLoad(source_texture, src_coords + vec2i(1, 1)));
    let c3_ump = srgb_to_linear(textureLoad(source_texture, src_coords + vec2i(1, 0)));

    let c0 = vec4f(c0_ump.rgb, 1) * c0_ump.a;
    let c1 = vec4f(c1_ump.rgb, 1) * c1_ump.a;
    let c2 = vec4f(c2_ump.rgb, 1) * c2_ump.a;
    let c3 = vec4f(c3_ump.rgb, 1) * c3_ump.a;

    let fnl = (c0 + c1 + c2 + c3) / 4;
    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(fnl.rgb / fnl.a, fnl.a)), vec4f(0), fnl.a < 1e-6));
}
