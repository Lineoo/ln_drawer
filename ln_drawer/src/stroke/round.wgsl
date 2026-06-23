// include! colorspace, dispatch

struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
}

@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;
@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if !(area_satisfied(id) && coords_satisfied(id)) { return; }

    var working_color = srgb_to_linear(textureLoad(destination, coords(id)));
    for (var i = 0u; i < draws_length; i++) {
        let raw_color = draws_array[i].color;
        let color = vec4f(raw_color.rgb, raw_color.a * draws_array[i].flow * smoothstep(
            (1.0 + draws_array[i].softness) * draws_array[i].size + 0.5,
            (1.0 - draws_array[i].softness) * draws_array[i].size + 0.5,
            length(
                vec2f(draws_array[i].position - area(id)) - vec2f(0.5) +
                vec2f(draws_array[i].position_fract) / vec2f(0xffffffff)
            ),
        ));

        if color.a < 1e-6 {
            continue;
        }

        let result = color.a * vec4f(color.rgb, 1) + (1 - color.a) * working_color.a * vec4f(working_color.rgb, 1);
        working_color = vec4f(result.rgb / result.a, result.a);
    }

    textureStore(destination, coords(id), linear_to_srgb(working_color));
}