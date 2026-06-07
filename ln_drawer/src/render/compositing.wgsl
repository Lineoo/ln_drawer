@group(0) @binding(0) var texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    return vec4f(f32(index / 2) * 2 - 1, f32(index % 2) * 2 - 1, 0, 1);
}

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    return textureLoad(texture, vec2i(floor(position.xy)), 0);
}
