struct DispatchMeta {
    dispatch_coords: vec2i,
    dispatch_size: vec2u,
}

struct Draw {
    color: vec4f,
    position: vec2i,
    position_fract: vec2u,
    softness: f32,
    size: f32,
    flow: f32,
}

@group(0) @binding(0) var<uniform> dispatch: DispatchMeta;
@group(0) @binding(1) var<uniform> draws_length: u32;
@group(0) @binding(2) var<storage, read> draws_array: array<Draw>;

@group(1) @binding(0) var destination: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> destination_key: vec3i;

const texture_size: i32 = 512;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let texl_size = i32(exp2(f32(destination_key.z)));
    let real_size = texture_size * texl_size;
    let chunk_min = (destination_key.xy) * real_size;
    let chunk_max = (destination_key.xy + vec2i(1)) * real_size;

    let area_min = dispatch.dispatch_coords;
    let area_max = dispatch.dispatch_coords + vec2i(dispatch.dispatch_size);
    let area = area_min + vec2i(id.xy) * texl_size;
    if area.x < area_min.x || area.y < area_min.y || area.x >= area_max.x || area.y >= area_max.y { return; }

    let coords_min = (real_size + area_min - chunk_min) / texl_size - texture_size;
    let coords_max = (real_size + area_max - chunk_min - 1) / texl_size + 1 - texture_size;
    let coords = coords_min + vec2i(id.xy);
    if coords.x >= coords_max.x || coords.y >= coords_max.y { return; }

    var working_color = srgb_to_linear(textureLoad(destination, coords));
    for (var i = 0u; i < draws_length; i++) {
        let raw_color = draws_array[i].color;
        let color = vec4f(raw_color.rgb, raw_color.a * draws_array[i].flow * smoothstep(
            1.0 + draws_array[i].softness,
            1.0 - draws_array[i].softness,
            length(
                vec2f(draws_array[i].position - area) - vec2f(0.5) +
                vec2f(draws_array[i].position_fract) / vec2f(0xffffffff)
            ) / draws_array[i].size,
        ));

        if color.a < 1e-6 {
            continue;
        }

        let result = color.a * vec4f(color.rgb, 1) + (1 - color.a) * working_color.a * vec4f(working_color.rgb, 1);
        working_color = vec4f(result.rgb / result.a, result.a);
    }

    textureStore(destination, coords, linear_to_srgb(working_color));
    // textureStore(texture, coords, vec4f(f32(dispatch.stroke_count) / 200, 0, 0, 1));
}

// Sorry but i dont really understand the srgb-thing here

fn srgb_to_linear(v: vec4f) -> vec4f {
    let threshold = vec3(0.04045);
    let low = v.rgb / 12.92;
    let high = pow((v.rgb + 0.055) / 1.055, vec3(2.4));
    return vec4f(select(high, low, v.rgb < threshold), v.a);
}

fn linear_to_srgb(v: vec4f) -> vec4f {
    let threshold = vec3(0.0031308);
    let low = v.rgb * 12.92;
    let high = 1.055 * pow(v.rgb, vec3(1.0 / 2.4)) - 0.055;
    return vec4f(select(high, low, v.rgb < threshold), v.a);
}