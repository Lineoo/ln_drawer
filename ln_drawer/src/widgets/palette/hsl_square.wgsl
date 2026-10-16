#lib_colorspace

@group(1) @binding(1) var<uniform> hsl: vec4f;

@fragment
fn main(@location(0) uv: vec2f) -> @location(0) vec4f {
    return srgb_gamma_decode(vec4f(hsl_to_rgb(hsl.x, uv.x, uv.y), 1.0));
}
