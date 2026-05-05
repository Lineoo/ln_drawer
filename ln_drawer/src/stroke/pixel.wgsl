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

    var working_color = textureLoad(texture, coords);
    for (var i = 0u; i < dispatch.stroke_count; i++) {
        let center = (draws_array[i].position - rect.origin);

        let a = draws_array[i].color;
        let color_a = a.rgb;
        let alpha_a = a.a * step(-1.0, -length(
            vec2f(center - coords) - vec2f(0.5) +
            vec2f(draws_array[i].position_fract) / vec2f(0xffffffff)
        ) / draws_array[i].size);

        if alpha_a < 1e-6 {
            continue;
        }

        let b = working_color;
        let color_b = b.rgb;
        let alpha_b = b.a;

        let alpha_result = alpha_a + alpha_b - alpha_a * alpha_b;
        let color_result = (alpha_a * color_a + (1 - alpha_a) * alpha_b * color_b) / alpha_result;
        working_color = vec4f(color_result, alpha_result);
    }

    textureStore(texture, coords, working_color);
    // textureStore(texture, coords, vec4f(f32(dispatch.stroke_count) / 200, 0, 0, 1));
}