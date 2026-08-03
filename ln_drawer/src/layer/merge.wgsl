struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(0) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read>;
@group(0) @binding(1) var<uniform> destination: Rectangle;

@group(1) @binding(0) var source_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> source: Rectangle;

@group(2) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var<uniform> swap: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    // Assumption: three textures are all the same pixel size
    // Expectation: Will merge all texel based on swap texture
    let position = swap.coords + vec2i(id.xy);

    if (any(position < destination.coords)) { return; }
    if (any(position >= destination.coords + vec2i(destination.size))) { return; }
    
    if (any(position < swap.coords)) { return; }
    if (any(position >= swap.coords + vec2i(swap.size))) { return; }

    let src_coords = position - source.coords;
    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    let src_satisfied = any(position < source.coords)
        || any(position >= source.coords + vec2i(source.size));

    let dst_ump = textureLoad(destination_texture, dst_coords);
    let src_ump = select(textureLoad(source_texture, src_coords), vec4f(), src_satisfied);

    let dst = vec4f(dst_ump.rgb, 1) * dst_ump.a;
    let src = vec4f(src_ump.rgb, 1) * src_ump.a;

    let swp = composite(src, dst);

    textureStore(swap_texture, swp_coords, select(vec4f(swp.rgb / swp.a, swp.a), vec4f(), swp.a < 1e-6));
}
