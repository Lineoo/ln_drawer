struct Rectangle {
    origin: vec2i,
    extend: vec2u,
}

struct Draw {
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

    let color_a = vec4f(0.0, 0.0, 0.0, 1.0);
    let color_b = textureLoad(texture, coords);

    let alpha = smoothstep(
        1.0 + brush.softness,
        1.0 - brush.softness,
        length(vec2f(center - coords)) / brush.size
    );

    let color = alpha * color_a + (1 - alpha) * color_b;
    textureStore(texture, coords, color);
}