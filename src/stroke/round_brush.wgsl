struct Brush {
    position: vec2f,
    size: f32,
    softness: f32,
}

@group(0) @binding(0) var texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(0) var<uniform> brush: Brush;

@compute @workgroup_size(8, 8)
fn round_brush(@builtin(global_invocation_id) id: vec3u) {
    let coords = vec2u(floor(brush.position)) - vec2u(16) + id.xy;
    let here = vec2f(coords) + vec2f(0.5);

    let color_a = vec4f(0.0, 0.0, 0.0, 1.0);
    let color_b = textureLoad(texture, coords);

    let alpha = smoothstep(
        1.0 + brush.softness,
        1.0 - brush.softness,
        distance(here, brush.position) / brush.size
    );

    let color = alpha * color_a + (1 - alpha) * color_b;
    textureStore(texture, coords, color);
}