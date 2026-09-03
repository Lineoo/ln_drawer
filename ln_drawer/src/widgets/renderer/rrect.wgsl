struct Quad {
    origin: vec2i,
    extend: vec2u,
    edge: vec2u,
}

struct RRect {
    color: vec4f,
    radius: f32,
    width: f32,
};

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@group(1) @binding(0) var<uniform> quad: Quad;
@group(1) @binding(1) var<uniform> rrect: RRect;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4f {
    let worldPos = vec2f(quad.origin) + in.uv * vec2f(quad.extend);
    let center = vec2f(quad.origin) + vec2f(quad.extend) * 0.5;
    let halfSize = vec2f(quad.extend) * 0.5;

    let d = sdRoundRect(worldPos - center, halfSize, rrect.radius);

    let width = max(rrect.width, fwidth(d));
    let half = width * 0.5;

    let alpha = 1.0 - smoothstep(-half, half, d);
    return rrect.color * alpha;
}

fn sdRoundRect(p: vec2f, b: vec2f, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec2f())) - r;
}