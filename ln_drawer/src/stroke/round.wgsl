struct Rectangle {
    origin: vec2i,
    extend: vec2u,
}

struct DispatchMeta {
    dirty_coords: vec2i,
    stroke_count: u32,
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
@group(0) @binding(1) var<storage, read> draws_array: array<Draw>;

@group(1) @binding(0) var texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(1) var<uniform> rect: Rectangle;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let coords = dispatch.dirty_coords + vec2i(id.xy) - rect.origin;

    var working_color = linear_to_srgb(textureLoad(texture, coords));
    for (var i = 0u; i < dispatch.stroke_count; i++) {
        let center = (draws_array[i].position - rect.origin);

        let raw_color = linear_to_srgb(linear_to_srgb(draws_array[i].color));
        let color = vec4f(raw_color.rgb, raw_color.a * draws_array[i].flow * smoothstep(
            1.0 + draws_array[i].softness,
            1.0 - draws_array[i].softness,
            length(
                vec2f(center - coords) - vec2f(0.5) +
                vec2f(draws_array[i].position_fract) / vec2f(0xffffffff)
            ) / draws_array[i].size,
        ));

        if color.a < 1e-6 {
            continue;
        }

        let result = color.a * vec4f(color.rgb, 1) + (1 - color.a) * working_color.a * vec4f(working_color.rgb, 1);
        working_color = vec4f(result.rgb / result.a, result.a);
    }

    textureStore(texture, coords, srgb_to_linear(working_color));
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