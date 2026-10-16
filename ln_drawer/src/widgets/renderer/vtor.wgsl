#lib_camera

struct Quad {
    origin: vec2i,
    extend: vec2u,
}

struct VertexInput {
    @location(0) pos: vec2f,
}

struct VertexOutput {
    @builtin(position) pos: vec4f,
}

@group(1) @binding(0) var<uniform> quad: Quad;
@group(1) @binding(1) var<uniform> color: vec4f;

@vertex
fn vs_main(@builtin(vertex_index) index: u32, in: VertexInput) -> VertexOutput {
    var ret: VertexOutput;
    ret.pos = vec4f(world_to_clip(quad.origin) + world_to_clip_relative(vec2f(quad.extend) * in.pos), 0.0, 1.0);
    return ret;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return color;
}
