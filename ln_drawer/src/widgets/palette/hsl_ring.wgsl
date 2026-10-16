#lib_colorspace #lib_constant

struct HslRing {
    hsl: vec4f,
    inner_radius: f32,
};

@group(1) @binding(1) var<uniform> ring: HslRing;

@fragment
fn main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let delta = uv - vec2f(0.5);
    let hue = fract(atan2(delta.y, delta.x) / TAU + 1.0);

    let color = srgb_gamma_decode(vec4f(hsl_to_rgb(hue, ring.hsl.y, ring.hsl.z), 1.0));
    return color * ring_alpha(delta);
}

fn ring_alpha(delta: vec2f) -> f32 {
    let radius = length(delta);
    let width = max(1e-6, fwidth(radius) * 0.5);
    let inner = smoothstep(ring.inner_radius - width, ring.inner_radius + width, radius);
    let outer = 1.0 - smoothstep(0.5 - width, 0.5 + width, radius);
    return min(inner, outer);
}
