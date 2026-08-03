struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
}

struct Rectangle {
    coords: vec2i,
    size: vec2u,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var<uniform> swap: Rectangle;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let position = dispatch.coords + vec2i(id.xy);

    if (any(position < dispatch.coords)) { return; }
    if (any(position - dispatch.coords >= vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position - destination.coords >= vec2i(destination.size))) { return; }
    
    if (any(position < swap.coords)) { return; }
    if (any(position - swap.coords >= vec2i(swap.size))) { return; }

    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    let dst_ump = textureLoad(destination_texture, dst_coords);
    var dst = vec4f(dst_ump.rgb, 1) * dst_ump.a;
    for (var i = 0u; i < draws_length; i++) {
        let src = vec4f(draws_array[i].color.rgb, 1) * draws_array[i].color.a * draws_array[i].flow * smoothstep(
            (1.0 + draws_array[i].softness) * draws_array[i].size + 0.5,
            (1.0 - draws_array[i].softness) * draws_array[i].size + 0.5,
            length(
                vec2f(draws_array[i].position - position) - vec2f(0.5) +
                vec2f(draws_array[i].position_fract) / vec2f(0xffffffff)
            ),
        );

        dst = composite(src, dst);
    }

    textureStore(swap_texture, swp_coords, select(vec4f(dst.rgb / dst.a, dst.a), vec4f(), dst.a < 1e-6));
}