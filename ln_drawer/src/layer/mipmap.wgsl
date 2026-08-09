// include! colorspace rectangle

@group(0) @binding(0) var<uniform> dispatch: Rectangle;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(2) @binding(1) var<uniform> source: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: source texture is always aligned sub-mipmap texture
    let scale = source.size / textureDimensions(source_texture) * 2;
    let start = max(max(dispatch.coords, destination.coords), source.coords);
    let position = start + vec2i(id.xy * scale);

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position)
        && rectangle_contains(source, position);
    if !validated { return; }

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

    let dst = (c0 + c1 + c2 + c3) / 4;
    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(dst.rgb / dst.a, dst.a)), vec4f(), dst.a < 1e-6));
}
