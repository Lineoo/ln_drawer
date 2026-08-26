#lib_colorspace

@group(1) @binding(1) var<uniform> oklab: vec4f;

@fragment
fn main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let color = oklab_to_linear_srgb(vec3f(oklab.x, uv.xy * 0.8 - 0.4));
    return select(vec4f(vec3f(0), 0.1), vec4f(color, 1), all(color <= vec3f(1) & color >= vec3f(0)));
}