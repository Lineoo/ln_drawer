@group(0) @binding(0) var texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    return vec4f(-1 + 4 * f32(index % 2), -1 + 4 * f32(index / 2), 0, 1);
}

@fragment
fn fs_main(@builtin(position) coord: vec4f) -> @location(0) vec4f {
    let color = textureLoad(texture, vec2u(floor(coord.xy)), 0);
    return srgb_to_linear(color + vec4f(1 - color.a));
}