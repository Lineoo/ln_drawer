// include! colorspace

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

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let scale = destination.size / textureDimensions(destination_texture);
    let position = dispatch.coords + vec2i(id.xy * scale);

    if (any(position >= dispatch.coords + vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position >= destination.coords + vec2i(destination.size))) { return; }

    let dst_coords = (position - destination.coords) / vec2i(scale);
    let dst_ump = srgb_to_linear(textureLoad(destination_texture, dst_coords));
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

        dst = src + dst * (1 - src.a);
    }

    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(dst.rgb / dst.a, dst.a)), vec4f(0), dst.a < 1e-6));
}

@compute @workgroup_size(16, 16)
fn cs_erase(@builtin(global_invocation_id) id: vec3u) {
    let scale = destination.size / textureDimensions(destination_texture);
    let position = dispatch.coords + vec2i(id.xy * scale);

    if (any(position >= dispatch.coords + vec2i(dispatch.size))) { return; }

    if (any(position < destination.coords)) { return; }
    if (any(position >= destination.coords + vec2i(destination.size))) { return; }

    let dst_coords = (position - destination.coords) / vec2i(scale);
    let dst_ump = srgb_to_linear(textureLoad(destination_texture, dst_coords));
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

        dst = dst * (1 - src.a);
    }

    textureStore(destination_texture, dst_coords, select(linear_to_srgb(vec4f(dst.rgb / dst.a, dst.a)), vec4f(0), dst.a < 1e-6));
}