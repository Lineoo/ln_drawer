#lib_rectangle #lib_colorspace

struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
}

@group(0) @binding(0) var<uniform> dispatch: Rectangle;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;

@group(1) @binding(0) var destination_texture: texture_storage_2d<rgba8unorm, #read>;
@group(1) @binding(1) var<uniform> destination: Rectangle;

@group(2) @binding(0) var swap_texture: texture_storage_2d<rgba8unorm, #write>;
@group(2) @binding(1) var<uniform> swap: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let start = max(max(dispatch.coords, destination.coords), swap.coords);
    let position = start + vec2i(id.xy);

    let validated = rectangle_contains(dispatch, position)
        && rectangle_contains(destination, position)
        && rectangle_contains(swap, position);
    if !validated { return; }

    let dst_coords = position - destination.coords;
    let swp_coords = position - swap.coords;

    var dst = sensitive_blend_encode(textureLoad(destination_texture, dst_coords));
    for (var i = 0u; i < draws_length; i++) {
        let draw = draws_array[i];
        let dist = length(vec2f(draw.position - position) - vec2f(0.5) + vec2f(draw.position_fract) * 0x1p-32);
        let mask = smoothstep((1.0 + draw.softness) * draw.size + 0.5, (1.0 - draw.softness) * draw.size + 0.5, dist);
        let src = mul_alpha(draw.color) * draw.flow * mask;

        dst = #composite;
    }

    textureStore(swap_texture, swp_coords, sensitive_blend_decode(dst));
}