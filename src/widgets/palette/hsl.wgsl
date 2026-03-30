struct Palette {
    h: f32,
    s: f32,
    l: f32,
};

@group(1) @binding(1) var<uniform> palette: Palette;

struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4f {
    return vec4f(hsl_to_rgb(vec3f(palette.h, in.uv.x, in.uv.y)), 1.0);
}

fn hsl_to_rgb(hsl: vec3f) -> vec3f {
    return hsl.z + hsl.y * (hue_to_rgb(hsl.x) - 0.5) * (1.0 - abs(2.0 * hsl.z - 1.0));
}

fn hue_to_rgb(h: f32) -> vec3f {
    return clamp(abs(((h * 6.0 + vec3f(0.0, 4.0, 2.0)) % 6.0) - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
}