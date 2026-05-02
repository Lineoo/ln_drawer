struct Rectangle {
    origin: vec2i,
    extend: vec2u,
}

struct Draw {
    dirty_coords: vec2i,
    stroke_count: u32,
}

struct Brush {
    color: vec4f,
    position: vec2i,
    force: f32,
    size: f32,
    softness: f32,
    flow: f32,
}

@group(0) @binding(0) var texture: texture_storage_2d<rgba8unorm, read_write>;
@group(0) @binding(1) var<uniform> rect: Rectangle;

@group(1) @binding(0) var<uniform> draw: Draw;

@group(2) @binding(0) var<storage, read> brush: array<Brush>;

@compute @workgroup_size(16, 16)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let coords = draw.dirty_coords + vec2i(id.xy) - rect.origin;

    var working_color = textureLoad(texture, coords);
    for (var i = 0u; i < draw.stroke_count; i++) {
        let center = (brush[i].position - rect.origin);

        let a = brush[i].color;
        let color_a = a.rgb;
        let alpha_a = a.a * brush[i].flow * smoothstep(
            1.0 + brush[i].softness,
            1.0 - brush[i].softness,
            length(vec2f(center - coords)) / brush[i].size,
        );

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
}