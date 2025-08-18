struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

@group(0) @binding(0)
var texture: texture_2d<f32>;

@group(0) @binding(1)
var texture_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x = f32(in_vertex_index & 1) * 4.0 - 1.0;
    let y = f32(in_vertex_index >> 1) * 4.0 - 1.0;
    var out: VertexOutput;
    out.uv = vec2<f32>(x, y);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(texture, texture_sampler, vec2<f32>((uv.x + 1.0) / 2.0, (uv.y + 1.0) / 2.0 * -1.0));
}