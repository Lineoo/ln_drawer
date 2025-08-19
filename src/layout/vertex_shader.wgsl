struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vertex.position.xy;
    out.color = vertex.color;
    out.position = vec4(vertex.position, 1.0);
    return out;
}

@fragment
fn fs_main(@location(1) color: vec3<f32>) -> @location(0) vec4<f32> {
    return vec4(color, 1.0);
}