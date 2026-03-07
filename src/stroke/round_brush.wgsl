struct Rectangle {
    origin: vec2i,
    extend: vec2u,
}

struct Draw {
    color: vec4f,
    position: vec2i,
    force: f32,
}

struct Brush {
    size: f32,
    softness: f32,
}

@group(0) @binding(0) var texture: texture_storage_2d<rgba8unorm, read_write>;
@group(0) @binding(1) var<uniform> rect: Rectangle;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var<uniform> brush: Brush;

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    let center = (draw.position - rect.origin);
    let coords = center - vec2i(16) + vec2i(id.xy);

    let a = draw.color;
    let color_a = a.rgb;
    let alpha_a = a.a * smoothstep(
        1.0 + brush.softness,
        1.0 - brush.softness,
        length(vec2f(center - coords)) / brush.size,
    );

    let b = textureLoad(texture, coords);
    let color_b = b.rgb;
    let alpha_b = b.a;

    let alpha_result = alpha_a + alpha_b - alpha_a * alpha_b;
    let color_result = (alpha_a * color_a + (1 - alpha_a) * alpha_b * color_b) / alpha_result;
    let result = vec4f(color_result, alpha_result);

    textureStore(texture, coords, result);
}